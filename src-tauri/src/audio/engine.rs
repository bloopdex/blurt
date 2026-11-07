//! The audio engine thread. cpal streams aren't `Send`, so one thread owns them and everything
//! else talks to it through `Cmd`s. It runs two sinks:
//!
//! * **virtual mic** – the virtual cable output that other apps hear as a microphone
//!   (plus optional passthrough of the real mic);
//! * **monitor** – your speakers/headphones, so you hear what you play. Previews go only here.

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::StreamTrait;
use serde::Serialize;

use super::decode::Pcm;
use super::devices::{find_input, find_output, open_input, open_output, pick_config};
use super::mixer::{AtomicF32, SinkRenderer, Voice, VoiceShared};
use super::passthrough::{MicReader, push_input, ring};

const POLL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq)]
pub struct EngineConfig {
    pub virtual_mic: Option<String>,
    /// Recording side of the virtual mic; passthrough must never read from it (feedback loop).
    pub virtual_mic_capture: Option<String>,
    pub monitor_device: Option<String>,
    pub mic_device: Option<String>,
    pub mic_passthrough: bool,
    pub sample_rate: Option<u32>,
    pub max_voices: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkKind {
    VirtualMic,
    Monitor,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SinkStatus {
    pub device: Option<String>,
    pub open: bool,
    pub sample_rate: Option<u32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub virtual_mic: SinkStatus,
    pub monitor: SinkStatus,
    pub passthrough: SinkStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceInfo {
    pub voice_id: u64,
    pub sound_id: Option<String>,
    pub name: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub paused: bool,
    pub preview: bool,
    pub on_mic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    Restart,
    Toggle,
    Overlap,
}

pub struct PlayRequest {
    pub sound_id: Option<String>,
    pub name: String,
    pub mic_pcm: Option<Arc<Pcm>>,
    pub monitor_pcm: Option<Arc<Pcm>>,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub gain: f32,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    pub mode: PlayMode,
    /// Stop every other (non-preview) sound first.
    pub exclusive: bool,
    pub preview: bool,
}

pub enum EngineEvent {
    Voices(Vec<VoiceInfo>),
    Status(EngineStatus),
    /// The set of audio devices or the system defaults changed.
    DevicesChanged,
}

enum Cmd {
    Configure(EngineConfig),
    Play(Box<PlayRequest>, Sender<Option<u64>>),
    Stop(u64),
    StopAll { previews_too: bool },
    Pause { voice: Option<u64>, paused: Option<bool> },
    SinkFailed(SinkKind, String),
}

pub struct Volumes {
    pub master: Arc<AtomicF32>,
    pub monitor: Arc<AtomicF32>,
    pub mic: Arc<AtomicF32>,
    /// Combined level of everything sent to the virtual mic.
    pub output: Arc<AtomicF32>,
}

pub struct Engine {
    tx: Sender<Cmd>,
    pub volumes: Volumes,
    status: Arc<Mutex<EngineStatus>>,
    voices: Arc<Mutex<Vec<VoiceInfo>>>,
}

impl Engine {
    pub fn start(
        config: EngineConfig,
        volumes: Volumes,
        on_event: impl Fn(EngineEvent) + Send + 'static,
    ) -> Self {
        let (tx, rx) = channel();
        let status = Arc::new(Mutex::new(EngineStatus::default()));
        let voices = Arc::new(Mutex::new(Vec::new()));
        let (worker_tx, worker_config) = (tx.clone(), config.clone());
        let worker_volumes =
            Volumes { master: volumes.master.clone(), monitor: volumes.monitor.clone(), mic: volumes.mic.clone(), output: volumes.output.clone() };
        let (worker_status, worker_voices) = (status.clone(), voices.clone());
        // Built on the audio thread itself: it will own cpal streams, which aren't `Send`.
        std::thread::Builder::new()
            .name("blurt-audio".into())
            .spawn(move || {
                Worker {
                    tx: worker_tx,
                    config: worker_config,
                    volumes: worker_volumes,
                    mic_sink: None,
                    monitor_sink: None,
                    active: BTreeMap::new(),
                    next_id: 1,
                    status: worker_status,
                    voices: worker_voices,
                    on_event: Box::new(on_event),
                    last_retry: Instant::now(),
                    // Taken at the first poll: enumerating devices here delayed opening them at launch.
                    fingerprint: String::new(),
                    last_poll: Instant::now(),
                }
                .run(rx)
            })
            .expect("spawn audio thread");
        tx.send(Cmd::Configure(config)).ok();
        Self { tx, volumes, status, voices }
    }

    pub fn configure(&self, config: EngineConfig) {
        self.send(Cmd::Configure(config));
    }

    /// Returns the new voice id, or `None` if nothing played (toggled off, or no device open).
    pub fn play(&self, req: PlayRequest) -> Option<u64> {
        let (reply_tx, reply_rx) = channel();
        self.send(Cmd::Play(Box::new(req), reply_tx));
        reply_rx.recv_timeout(Duration::from_secs(2)).ok().flatten()
    }

    pub fn stop(&self, voice_id: u64) {
        self.send(Cmd::Stop(voice_id));
    }

    pub fn stop_all(&self, previews_too: bool) {
        self.send(Cmd::StopAll { previews_too });
    }

    pub fn set_paused(&self, voice: Option<u64>, paused: Option<bool>) {
        self.send(Cmd::Pause { voice, paused });
    }

    pub fn status(&self) -> EngineStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn voices(&self) -> Vec<VoiceInfo> {
        self.voices.lock().map(|v| v.clone()).unwrap_or_default()
    }

    fn send(&self, cmd: Cmd) {
        if self.tx.send(cmd).is_err() {
            log::error!("audio thread has stopped; command dropped");
        }
    }
}

struct Sink {
    _stream: cpal::Stream,
    _input: Option<cpal::Stream>,
    tx: Sender<Voice>,
    rate: u32,
}

struct Active {
    sound_id: Option<String>,
    name: String,
    preview: bool,
    stopping: bool,
    start: usize,
    end: usize,
    rate: u32,
    on_mic: bool,
    shared: Vec<(SinkKind, Arc<VoiceShared>)>,
}

struct Worker {
    tx: Sender<Cmd>,
    config: EngineConfig,
    volumes: Volumes,
    mic_sink: Option<Sink>,
    monitor_sink: Option<Sink>,
    active: BTreeMap<u64, Active>,
    next_id: u64,
    status: Arc<Mutex<EngineStatus>>,
    voices: Arc<Mutex<Vec<VoiceInfo>>>,
    on_event: Box<dyn Fn(EngineEvent) + Send>,
    last_retry: Instant,
    fingerprint: String,
    last_poll: Instant,
}

impl Worker {
    fn run(mut self, rx: Receiver<Cmd>) {
        let mut was_active = false;
        loop {
            let timeout = if !self.active.is_empty() {
                Duration::from_millis(100)
            } else if self.needs_retry() {
                Duration::from_secs(2)
            } else {
                POLL
            };
            match rx.recv_timeout(timeout) {
                Ok(cmd) => self.handle(cmd),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if self.last_poll.elapsed() >= POLL {
                self.poll_devices();
            }
            if self.needs_retry() && self.last_retry.elapsed() >= Duration::from_secs(2) {
                self.open_missing();
            }
            self.reap();
            if !self.active.is_empty() || was_active {
                self.publish_voices();
            }
            was_active = !self.active.is_empty();
        }
    }

    fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Configure(cfg) => {
                let devices_changed = cfg.virtual_mic != self.config.virtual_mic
                    || cfg.monitor_device != self.config.monitor_device
                    || cfg.mic_device != self.config.mic_device
                    || cfg.virtual_mic_capture != self.config.virtual_mic_capture
                    || cfg.mic_passthrough != self.config.mic_passthrough
                    || cfg.sample_rate != self.config.sample_rate;
                let first = self.mic_sink.is_none() && self.monitor_sink.is_none();
                self.config = cfg;
                if devices_changed || first {
                    self.drop_sink(SinkKind::VirtualMic);
                    self.drop_sink(SinkKind::Monitor);
                    self.update_status(SinkKind::VirtualMic, |s| *s = SinkStatus::default());
                    self.update_passthrough(|s| *s = SinkStatus::default());
                    self.open_missing();
                }
            }
            Cmd::Play(req, reply) => {
                let id = self.play(*req);
                reply.send(id).ok();
            }
            Cmd::Stop(id) => self.stop(id),
            Cmd::StopAll { previews_too } => {
                let ids: Vec<u64> =
                    self.active.iter().filter(|(_, a)| previews_too || !a.preview).map(|(id, _)| *id).collect();
                ids.into_iter().for_each(|id| self.stop(id));
            }
            Cmd::Pause { voice, paused } => {
                let all_paused = self
                    .active
                    .values()
                    .filter(|a| !a.stopping)
                    .all(|a| a.shared.iter().all(|(_, s)| s.paused.load(Ordering::Relaxed)));
                for (id, a) in self.active.iter() {
                    if voice.is_some_and(|v| v != *id) {
                        continue;
                    }
                    let current = Self::live_handle(a).is_some_and(|s| s.paused.load(Ordering::Relaxed));
                    let target = paused.unwrap_or(if voice.is_some() { !current } else { !all_paused });
                    a.shared.iter().for_each(|(_, s)| s.paused.store(target, Ordering::Relaxed));
                }
            }
            Cmd::SinkFailed(kind, err) => {
                if self.drop_sink(kind) {
                    log::warn!("{kind:?} device failed: {err}");
                    self.update_status(kind, |s| {
                        s.open = false;
                        s.error = Some(format!("Device disconnected or stopped: {err}"));
                    });
                    if kind == SinkKind::VirtualMic {
                        self.update_passthrough(|s| s.open = false);
                    }
                    self.last_retry = Instant::now();
                }
            }
        }
    }

    /// Notice plug/unplug, driver (re)installs and default-device switches. Errors from a running
    /// stream are caught by its error callback; this catches everything that doesn't error.
    /// shortcut: polls every 5 s; switch to IMMNotificationClient / Core Audio listeners if that
    /// latency matters.
    fn poll_devices(&mut self) {
        self.last_poll = Instant::now();
        let fp = super::devices::fingerprint();
        if self.fingerprint.is_empty() {
            self.fingerprint = fp;
            return;
        }
        if fp == self.fingerprint {
            return;
        }
        // Field 2 of the fingerprint is the default output; a monitor on "system default" follows it.
        let default_out = |f: &str| f.split('#').nth(2).map(str::to_owned);
        let default_moved = default_out(&fp) != default_out(&self.fingerprint);
        self.fingerprint = fp;
        log::info!("audio devices changed");
        if default_moved && self.config.monitor_device.is_none() && self.drop_sink(SinkKind::Monitor) {
            self.update_status(SinkKind::Monitor, |s| s.open = false);
        }
        self.open_missing();
        (self.on_event)(EngineEvent::DevicesChanged);
    }

    fn needs_retry(&self) -> bool {
        self.config.virtual_mic.is_some() && self.mic_sink.is_none() || self.monitor_sink.is_none()
    }

    fn open_missing(&mut self) {
        self.last_retry = Instant::now();
        if self.mic_sink.is_none()
            && let Some(name) = self.config.virtual_mic.clone()
        {
            let result = self.open_mic_chain(&name);
            match result {
                Ok(sink) => {
                    let rate = sink.rate;
                    self.mic_sink = Some(sink);
                    self.update_status(SinkKind::VirtualMic, |s| {
                        *s = SinkStatus { device: Some(name.clone()), open: true, sample_rate: Some(rate), error: None }
                    });
                }
                Err(e) => self.update_status(SinkKind::VirtualMic, |s| {
                    *s = SinkStatus { device: Some(name.clone()), open: false, sample_rate: None, error: Some(e) }
                }),
            }
        }
        if self.monitor_sink.is_none() {
            let name = self.config.monitor_device.clone();
            let result = if name.is_some() && name == self.config.virtual_mic {
                Err("The monitor can't be the virtual microphone device — pick your speakers or headphones.".into())
            } else {
                self.open_sink(SinkKind::Monitor, name.as_deref(), self.volumes.monitor.clone(), None, None)
            };
            match result {
                Ok(sink) => {
                    let rate = sink.rate;
                    self.monitor_sink = Some(sink);
                    self.update_status(SinkKind::Monitor, |s| {
                        *s = SinkStatus { device: name.clone(), open: true, sample_rate: Some(rate), error: None }
                    });
                }
                Err(e) => self.update_status(SinkKind::Monitor, |s| {
                    *s = SinkStatus { device: name.clone(), open: false, sample_rate: None, error: Some(e) }
                }),
            }
        }
    }

    /// Virtual-mic output, with the physical mic mixed in when passthrough is on.
    fn open_mic_chain(&mut self, name: &str) -> Result<Sink, String> {
        let device = find_output(Some(name))?;
        let cfg = pick_config(&device, true, self.config.sample_rate)?;
        let out_rate = cfg.sample_rate().0;
        let mut input = None;
        let mut reader = None;
        if self.config.mic_passthrough {
            match self.open_passthrough(out_rate) {
                Ok((stream, r, mic_name, in_rate)) => {
                    input = Some(stream);
                    reader = Some(r);
                    self.update_passthrough(|s| {
                        *s = SinkStatus { device: Some(mic_name), open: true, sample_rate: Some(in_rate), error: None }
                    });
                }
                // The soundboard still works without passthrough; surface the problem instead of failing.
                Err(e) => self.update_passthrough(|s| {
                    *s = SinkStatus { device: None, open: false, sample_rate: None, error: Some(e) }
                }),
            }
        } else {
            self.update_passthrough(|s| *s = SinkStatus::default());
        }
        let mut sink = self.open_sink(SinkKind::VirtualMic, Some(name), self.volumes.master.clone(), reader, Some(cfg))?;
        sink._input = input;
        Ok(sink)
    }

    fn open_passthrough(&self, out_rate: u32) -> Result<(cpal::Stream, MicReader, String, u32), String> {
        use cpal::traits::DeviceTrait;
        let device = find_input(self.config.mic_device.as_deref())?;
        let mic_name = device.name().unwrap_or_else(|_| "Microphone".into());
        if self.config.virtual_mic_capture.as_deref() == Some(mic_name.as_str())
        {
            return Err("Passthrough can't use the virtual microphone as its source — that would loop.".into());
        }
        let cfg = pick_config(&device, false, Some(out_rate))?;
        let in_rate = cfg.sample_rate().0;
        let (mut producer, consumer) = ring(in_rate);
        let err_tx = self.tx.clone();
        let stream = open_input(
            &device,
            &cfg,
            move |data, ch| push_input(&mut producer, data, ch, |s| s),
            move |e| {
                err_tx.send(Cmd::SinkFailed(SinkKind::VirtualMic, format!("microphone: {e}"))).ok();
            },
        )?;
        stream.play().map_err(|e| format!("Couldn't start the microphone: {e}"))?;
        Ok((stream, MicReader::new(consumer, in_rate, out_rate, self.volumes.mic.clone()), mic_name, in_rate))
    }

    fn open_sink(
        &self,
        kind: SinkKind,
        name: Option<&str>,
        volume: Arc<AtomicF32>,
        mic: Option<MicReader>,
        cfg: Option<cpal::SupportedStreamConfig>,
    ) -> Result<Sink, String> {
        let device = find_output(name)?;
        let cfg = match cfg {
            Some(c) => c,
            None => pick_config(&device, true, self.config.sample_rate)?,
        };
        let (tx, rx) = channel();
        let post = (kind == SinkKind::VirtualMic).then(|| self.volumes.output.clone());
        let mut renderer = SinkRenderer::new(rx, volume, mic, post);
        let err_tx = self.tx.clone();
        let stream = open_output(
            &device,
            &cfg,
            move |buf, ch| renderer.render(buf, ch),
            move |e| {
                err_tx.send(Cmd::SinkFailed(kind, e.to_string())).ok();
            },
        )?;
        stream.play().map_err(|e| format!("Couldn't start the device: {e}"))?;
        log::info!("opened {kind:?} on {:?} at {} Hz", name.unwrap_or("default"), cfg.sample_rate().0);
        Ok(Sink { _stream: stream, _input: None, tx, rate: cfg.sample_rate().0 })
    }

    fn play(&mut self, req: PlayRequest) -> Option<u64> {
        if req.preview {
            let ids: Vec<u64> = self.active.iter().filter(|(_, a)| a.preview).map(|(id, _)| *id).collect();
            ids.into_iter().for_each(|id| self.stop(id));
        } else {
            if let Some(sid) = &req.sound_id {
                let existing: Vec<u64> = self
                    .active
                    .iter()
                    .filter(|(_, a)| !a.preview && !a.stopping && a.sound_id.as_ref() == Some(sid))
                    .map(|(id, _)| *id)
                    .collect();
                if !existing.is_empty() && req.mode != PlayMode::Overlap {
                    existing.into_iter().for_each(|id| self.stop(id));
                    if req.mode == PlayMode::Toggle {
                        return None;
                    }
                }
            }
            if req.exclusive {
                let ids: Vec<u64> = self.active.iter().filter(|(_, a)| !a.preview).map(|(id, _)| *id).collect();
                ids.into_iter().for_each(|id| self.stop(id));
            }
            loop {
                let live: Vec<u64> =
                    self.active.iter().filter(|(_, a)| !a.preview && !a.stopping).map(|(id, _)| *id).collect();
                if live.len() < self.config.max_voices.max(1) {
                    break;
                }
                self.stop(live[0]); // BTreeMap order = oldest first
            }
        }

        let mut shared = Vec::new();
        let mut timing = None;
        let mut on_mic = false;
        let targets = [
            (&self.mic_sink, &req.mic_pcm, SinkKind::VirtualMic),
            (&self.monitor_sink, &req.monitor_pcm, SinkKind::Monitor),
        ];
        for (sink, pcm, kind) in targets {
            let (Some(sink), Some(pcm)) = (sink, pcm) else { continue };
            if pcm.rate != sink.rate {
                log::warn!("dropping voice for a sink whose rate changed ({} != {})", pcm.rate, sink.rate);
                continue;
            }
            let ms_to_frames = |ms: i64| (ms.max(0) as u64 * pcm.rate as u64 / 1000) as usize;
            let start = ms_to_frames(req.start_ms);
            let end = req.end_ms.map(ms_to_frames).unwrap_or(pcm.frames());
            let voice = Voice::new(
                pcm.clone(),
                start,
                end,
                req.gain,
                ms_to_frames(req.fade_in_ms as i64),
                ms_to_frames(req.fade_out_ms as i64),
            );
            timing.get_or_insert((voice.start, voice.end, pcm.rate));
            let handle = voice.shared.clone();
            if sink.tx.send(voice).is_ok() {
                on_mic |= kind == SinkKind::VirtualMic;
                shared.push((kind, handle));
            }
        }
        let (start, end, rate) = timing?;
        let id = self.next_id;
        self.next_id += 1;
        self.active.insert(
            id,
            Active {
                sound_id: req.sound_id,
                name: req.name,
                preview: req.preview,
                stopping: false,
                start,
                end,
                rate,
                on_mic,
                shared,
            },
        );
        self.publish_voices();
        Some(id)
    }

    fn stop(&mut self, id: u64) {
        if let Some(a) = self.active.get_mut(&id) {
            a.stopping = true;
            a.shared.iter().for_each(|(_, s)| s.stopping.store(true, Ordering::Relaxed));
        }
    }

    fn reap(&mut self) {
        self.active.retain(|_, a| !a.shared.iter().all(|(_, s)| s.done.load(Ordering::Relaxed)));
    }

    /// A closed sink never renders again, so its voices must be marked finished by hand.
    fn drop_sink(&mut self, kind: SinkKind) -> bool {
        let slot = match kind {
            SinkKind::VirtualMic => &mut self.mic_sink,
            SinkKind::Monitor => &mut self.monitor_sink,
        };
        let had = slot.take().is_some();
        for a in self.active.values() {
            a.shared.iter().filter(|(k, _)| *k == kind).for_each(|(_, s)| s.done.store(true, Ordering::Relaxed));
        }
        had
    }

    /// The first handle still rendering drives the progress display.
    fn live_handle(a: &Active) -> Option<&Arc<VoiceShared>> {
        a.shared.iter().map(|(_, s)| s).find(|s| !s.done.load(Ordering::Relaxed))
    }

    fn publish_voices(&self) {
        let list: Vec<VoiceInfo> = self
            .active
            .iter()
            .filter(|(_, a)| !a.stopping)
            .map(|(id, a)| {
                let frame = Self::live_handle(a).map(|s| s.frame.load(Ordering::Relaxed)).unwrap_or(a.end);
                let ms = |frames: usize| (frames as u64 * 1000 / a.rate.max(1) as u64) as i64;
                VoiceInfo {
                    voice_id: *id,
                    sound_id: a.sound_id.clone(),
                    name: a.name.clone(),
                    position_ms: ms(frame.saturating_sub(a.start)),
                    duration_ms: ms(a.end - a.start),
                    paused: Self::live_handle(a).is_some_and(|s| s.paused.load(Ordering::Relaxed)),
                    preview: a.preview,
                    on_mic: a.on_mic,
                }
            })
            .collect();
        if let Ok(mut v) = self.voices.lock() {
            v.clone_from(&list);
        }
        (self.on_event)(EngineEvent::Voices(list));
    }

    fn update_status(&self, kind: SinkKind, f: impl FnOnce(&mut SinkStatus)) {
        let snapshot = {
            let mut st = self.status.lock().unwrap_or_else(|e| e.into_inner());
            let before = st.clone();
            f(match kind {
                SinkKind::VirtualMic => &mut st.virtual_mic,
                SinkKind::Monitor => &mut st.monitor,
            });
            (*st != before).then(|| st.clone())
        };
        if let Some(s) = snapshot {
            (self.on_event)(EngineEvent::Status(s));
        }
    }

    fn update_passthrough(&self, f: impl FnOnce(&mut SinkStatus)) {
        let snapshot = {
            let mut st = self.status.lock().unwrap_or_else(|e| e.into_inner());
            let before = st.clone();
            f(&mut st.passthrough);
            (*st != before).then(|| st.clone())
        };
        if let Some(s) = snapshot {
            (self.on_event)(EngineEvent::Status(s));
        }
    }
}
