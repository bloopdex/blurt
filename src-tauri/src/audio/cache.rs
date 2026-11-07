//! Decoded-audio cache keyed by (file, device sample rate), bounded by a byte budget.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::decode::{Pcm, decode_file, resample_stereo};
use crate::error::AppResult;

const BUDGET_BYTES: usize = 384 * 1024 * 1024;

struct Entry {
    pcm: Arc<Pcm>,
    last_used: u64,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<(PathBuf, u32), Entry>,
    bytes: usize,
    clock: u64,
}

#[derive(Default)]
pub struct AudioCache {
    inner: Mutex<Inner>,
}

impl AudioCache {
    /// Returns one buffer per requested rate, decoding the file at most once.
    pub fn get(&self, path: &Path, rates: &[u32]) -> AppResult<Vec<Arc<Pcm>>> {
        let mut out: Vec<Option<Arc<Pcm>>> = {
            let mut inner = self.lock();
            inner.clock += 1;
            let now = inner.clock;
            rates
                .iter()
                .map(|r| {
                    inner.entries.get_mut(&(path.to_path_buf(), *r)).map(|e| {
                        e.last_used = now;
                        e.pcm.clone()
                    })
                })
                .collect()
        };
        if out.iter().all(Option::is_some) {
            return Ok(out.into_iter().flatten().collect());
        }

        // Decode outside the lock so a long file doesn't stall other sounds.
        let decoded = decode_file(path)?.pcm;
        for (slot, &rate) in out.iter_mut().zip(rates) {
            if slot.is_none() {
                let pcm = Arc::new(Pcm {
                    samples: resample_stereo(&decoded.samples, decoded.rate, rate),
                    rate,
                    peak: decoded.peak,
                });
                self.insert(path, rate, pcm.clone());
                *slot = Some(pcm);
            }
        }
        Ok(out.into_iter().flatten().collect())
    }

    fn insert(&self, path: &Path, rate: u32, pcm: Arc<Pcm>) {
        let mut inner = self.lock();
        inner.clock += 1;
        let size = pcm.samples.len() * 4;
        let last_used = inner.clock;
        if let Some(old) = inner.entries.insert((path.to_path_buf(), rate), Entry { pcm, last_used }) {
            inner.bytes -= old.pcm.samples.len() * 4;
        }
        inner.bytes += size;
        while inner.bytes > BUDGET_BYTES && inner.entries.len() > 1 {
            let oldest = inner.entries.iter().min_by_key(|(_, e)| e.last_used).map(|(k, _)| k.clone());
            let Some(k) = oldest else { break };
            if let Some(e) = inner.entries.remove(&k) {
                inner.bytes -= e.pcm.samples.len() * 4;
            }
        }
    }

    pub fn evict(&self, path: &Path) {
        let mut inner = self.lock();
        let keys: Vec<_> = inner.entries.keys().filter(|(p, _)| p == path).cloned().collect();
        for k in keys {
            if let Some(e) = inner.entries.remove(&k) {
                inner.bytes -= e.pcm.samples.len() * 4;
            }
        }
    }

    pub fn clear(&self) {
        let mut inner = self.lock();
        inner.entries.clear();
        inner.bytes = 0;
    }

    pub fn bytes(&self) -> usize {
        self.lock().bytes
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::decode::tests::{sine, write_wav};

    #[test]
    fn caches_per_rate_and_evicts() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.wav");
        write_wav(&p, 44100, 1, &sine(44100, 0.2, 0.3));
        let cache = AudioCache::default();
        let bufs = cache.get(&p, &[48000, 44100]).unwrap();
        assert_eq!((bufs[0].rate, bufs[1].rate), (48000, 44100));
        assert_eq!(bufs[0].frames(), 9600);
        let again = cache.get(&p, &[48000]).unwrap();
        assert!(Arc::ptr_eq(&bufs[0], &again[0]), "second lookup is served from cache");
        assert!(cache.bytes() > 0);
        cache.evict(&p);
        assert_eq!(cache.bytes(), 0);
    }
}
