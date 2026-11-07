//! Tauri commands: the whole frontend ⇄ backend API. Anything that touches disk, decodes audio,
//! or runs a process is `async` + `spawn_blocking` so the UI thread never stalls.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::audio::devices::{self, DeviceList};
use crate::audio::endpoints;
use crate::audio::engine::{EngineStatus, VoiceInfo};
use crate::audio::routing::{self, VerifyResult};
use crate::db::downloads::{DownloadRecord, NewDownload};
use crate::db::groups::Group;
use crate::db::sounds::{Sound, SoundPatch};
use crate::desktop::shortcuts::{self, ShortcutProblem};
use crate::downloader::{Downloader, MediaInfo, sources, ytdlp};
use crate::error::{AppError, AppResult};
use crate::library::{self, ImportFailure, ImportOptions, ImportReport};
use crate::paths::dir_size;
use crate::playback::{self, PlayOutcome};
use crate::settings::Settings;
use crate::state::{AppState, engine_config, resolve_capture};

type Cmd<'a> = State<'a, AppState>;

async fn blocking<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppHandle, &AppState) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app, &app.state::<AppState>()))
        .await
        .map_err(|e| AppError::Audio(format!("Background task failed: {e}")))?
}

// ── Library ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    sounds: Vec<Sound>,
    groups: Vec<Group>,
}

#[tauri::command]
pub fn get_library(state: Cmd) -> AppResult<Library> {
    Ok(Library { sounds: state.db.list_sounds()?, groups: state.db.list_groups()? })
}

#[tauri::command]
pub async fn import_files(app: AppHandle, paths: Vec<String>, category_id: Option<String>) -> AppResult<ImportReport> {
    if paths.len() > 500 {
        return Err(AppError::Invalid("Import up to 500 files at a time.".into()));
    }
    blocking(app, move |_, state| {
        let volume = state.settings().default_sound_volume as f64;
        let mut report = ImportReport { imported: vec![], failed: vec![] };
        for p in paths {
            let opts = ImportOptions {
                name: None,
                category_id: category_id.clone(),
                volume,
                source_url: None,
                trim_start_ms: 0,
                trim_end_ms: None,
            };
            match library::import_file(&state.db, &state.paths, &PathBuf::from(&p), opts) {
                Ok(s) => report.imported.push(s),
                Err(e) => report.failed.push(ImportFailure { path: p, message: e.to_string() }),
            }
        }
        Ok(report)
    })
    .await
}

#[tauri::command]
pub fn update_sound(state: Cmd, app: AppHandle, id: String, patch: SoundPatch) -> AppResult<Sound> {
    let mode_changed = patch.playback_mode.is_some();
    let s = state.db.update_sound(&id, patch)?;
    if mode_changed && s.shortcut.is_some() {
        shortcuts::sync(&app); // hold-to-play is decided at registration
    }
    Ok(s)
}

#[tauri::command]
pub fn delete_sounds(state: Cmd, app: AppHandle, ids: Vec<String>) -> AppResult<()> {
    let mut had_shortcut = false;
    for id in &ids {
        had_shortcut |= state.db.get_sound(id).map(|s| s.shortcut.is_some()).unwrap_or(false);
        let path = library::delete(&state.db, &state.paths, id)?;
        state.cache.evict(&path);
    }
    if had_shortcut {
        shortcuts::sync(&app);
    }
    Ok(())
}

#[tauri::command]
pub async fn duplicate_sound(app: AppHandle, id: String) -> AppResult<Sound> {
    blocking(app, move |_, s| library::duplicate(&s.db, &s.paths, &id)).await
}

#[tauri::command]
pub fn set_sound_cover(state: Cmd, id: String, path: Option<String>) -> AppResult<Sound> {
    library::set_cover(&state.db, &state.paths, &id, path.as_deref().map(std::path::Path::new))
}

#[tauri::command]
pub fn reorder_sounds(state: Cmd, ids: Vec<String>) -> AppResult<()> {
    state.db.reorder_sounds(&ids)
}

#[tauri::command]
pub fn move_sounds(state: Cmd, ids: Vec<String>, category_id: Option<String>) -> AppResult<()> {
    state.db.move_to_category(&ids, category_id.as_deref())
}

#[tauri::command]
pub fn set_collection_membership(state: Cmd, collection_id: String, ids: Vec<String>, member: bool) -> AppResult<()> {
    state.db.set_collection_membership(&collection_id, &ids, member)
}

#[tauri::command]
pub fn create_group(state: Cmd, kind: String, name: String, icon: String, color: String) -> AppResult<Group> {
    state.db.create_group(&kind, &name, &icon, &color)
}

#[tauri::command]
pub fn update_group(state: Cmd, id: String, name: String, icon: String, color: String) -> AppResult<()> {
    state.db.update_group(&id, &name, &icon, &color)
}

#[tauri::command]
pub fn delete_group(state: Cmd, id: String) -> AppResult<()> {
    state.db.delete_group(&id)
}

#[tauri::command]
pub fn reorder_groups(state: Cmd, ids: Vec<String>) -> AppResult<()> {
    state.db.reorder_groups(&ids)
}

// ── Playback ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn play_sound(app: AppHandle, id: String) -> AppResult<PlayOutcome> {
    blocking(app, move |app, state| playback::play_sound(app, state, &id, None)).await
}

#[tauri::command]
pub async fn preview_sound(
    app: AppHandle,
    id: String,
    start_ms: i64,
    end_ms: Option<i64>,
    volume: f32,
) -> AppResult<Option<u64>> {
    blocking(app, move |_, state| {
        let s = state.db.get_sound(&id)?;
        playback::preview(state, &state.paths.sound_file(&s.file_name), &s.name, start_ms, end_ms, volume.clamp(0.0, 2.0))
    })
    .await
}

/// Preview a file on disk before importing it (monitor only).
#[tauri::command]
pub async fn preview_file(app: AppHandle, path: String) -> AppResult<Option<u64>> {
    blocking(app, move |_, state| {
        let p = PathBuf::from(&path);
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("Preview").to_owned();
        playback::preview(state, &p, &name, 0, None, 1.0)
    })
    .await
}

#[tauri::command]
pub fn stop_voice(state: Cmd, voice_id: u64) {
    state.engine.stop(voice_id);
}

#[tauri::command]
pub async fn replay_last(app: AppHandle) -> AppResult<PlayOutcome> {
    blocking(app, playback::replay_last).await
}

#[tauri::command]
pub async fn save_clip(app: AppHandle) -> AppResult<Sound> {
    blocking(app, |app, _| crate::desktop::save_clip(app)).await
}

/// Speak `text` with the system voice into a new sound (then the UI plays it).
#[tauri::command]
pub async fn text_to_speech(app: AppHandle, text: String, voice: Option<String>) -> AppResult<Sound> {
    blocking(app, move |_, state| crate::tts::speak(state, &text, voice.as_deref())).await
}

#[tauri::command]
pub async fn tts_voices(app: AppHandle) -> AppResult<Vec<String>> {
    blocking(app, |_, _| Ok(crate::tts::voices())).await
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    crate::desktop::overlay::hide(&app);
}

#[tauri::command]
pub fn stop_all(state: Cmd) {
    state.engine.stop_all(true);
}

#[tauri::command]
pub fn set_paused(state: Cmd, voice_id: Option<u64>, paused: Option<bool>) {
    state.engine.set_paused(voice_id, paused);
}

#[tauri::command]
pub fn get_voices(state: Cmd) -> Vec<VoiceInfo> {
    state.engine.voices()
}

// ── Audio devices & routing ──────────────────────────────────────────────────

#[tauri::command]
pub async fn list_devices() -> AppResult<DeviceList> {
    tauri::async_runtime::spawn_blocking(devices::list).await.map_err(|e| AppError::Audio(e.to_string()))
}

#[tauri::command]
pub fn get_engine_status(state: Cmd) -> EngineStatus {
    state.engine.status()
}

/// Explain a device name that isn't where it should be, by its actual direction.
fn wrong_direction(name: &str, want_capture: bool, list: &DeviceList) -> Option<AppError> {
    let (want, other) = if want_capture { (&list.inputs, &list.outputs) } else { (&list.outputs, &list.inputs) };
    if want.iter().any(|d| d.name == name) {
        return None;
    }
    Some(AppError::Audio(if other.iter().any(|d| d.name == name) {
        if want_capture {
            format!("“{name}” is a playback device, not a microphone. Pick the recording side other apps listen to.")
        } else {
            format!("“{name}” is a recording device. Blurt needs a playback device to send sounds into.")
        }
    } else {
        format!("“{name}” isn't connected right now.")
    }))
}

/// Round trip through the configured route: tone into the playback device, listened for on the
/// recording device other apps use. The result is remembered per device pair.
/// With `fix_level`, a signal that arrives too quiet is made up by raising the recording
/// device's Windows input level by the measured loss, then tested again.
#[tauri::command]
pub async fn verify_virtual_mic(app: AppHandle, fix_level: Option<bool>) -> AppResult<VerifyResult> {
    blocking(app, move |_, state| {
        let s = state.settings();
        let Some(output) = s.virtual_mic_device.clone() else {
            return Err(AppError::Audio("Choose the playback device Blurt sends sounds into first.".into()));
        };
        let list = devices::list();
        if let Some(e) = wrong_direction(&output, false, &list) {
            return Err(e);
        }
        let Some(capture) = resolve_capture(&s) else {
            return Err(AppError::Audio(format!(
                "“{output}” has no recording side that Blurt can find. Choose the virtual microphone other apps should use."
            )));
        };
        if let Some(e) = wrong_direction(&capture, true, &list) {
            return Err(e);
        }
        test_pair(state, &output, &capture, fix_level == Some(true))
    })
    .await
}

/// Tone test of one pair; optionally makes up a quiet signal with the recording device's input level.
fn test_pair(state: &AppState, output: &str, capture: &str, fix_level: bool) -> AppResult<VerifyResult> {
    let mut result = routing::verify(output, capture).map_err(AppError::Audio)?;
    let loss = result.expected_db - result.received_db;
    // Leave headroom: some cables (Steam) drift a few dB between runs, and a full-scale sound must not clip.
    if fix_level && result.signal_received && loss > routing::FIX_HEADROOM_DB + 3.0 {
        let (before, after) = endpoints::raise_capture_level(capture, loss - routing::FIX_HEADROOM_DB).map_err(AppError::Audio)?;
        log::info!("raised input level of {capture:?} from {before:.1} dB to {after:.1} dB");
        result = routing::verify(output, capture).map_err(AppError::Audio)?;
    }
    state.record_route_check(&result)?;
    Ok(result)
}

/// One-click setup: tests every virtual device pair on this machine, fixes a quiet level, selects
/// the loudest working pair and mixes the user's real microphone in. Fails, changing nothing,
/// when no pair carries sound.
#[tauri::command]
pub async fn auto_setup_virtual_mic(app: AppHandle) -> AppResult<VerifyResult> {
    blocking(app, |app, state| {
        let list = devices::list();
        let mut best: Option<VerifyResult> = None;
        for (output, capture) in list.loopback_candidates() {
            let Ok(r) = test_pair(state, &output, &capture, true) else { continue };
            if r.signal_received && best.as_ref().is_none_or(|b| r.received_db > b.received_db) {
                best = Some(r);
            }
            if best.as_ref().is_some_and(|b| b.quality_ok) {
                break;
            }
        }
        let Some(best) = best else {
            return Err(AppError::Audio(
                "None of the virtual audio devices on this PC carry sound into a microphone. Install VB-CABLE (free, link below), restart, then run this again.".into(),
            ));
        };
        let s = state.settings();
        let mic = list.passthrough_source(s.mic_device.as_deref(), &best.capture);
        update_settings(
            app.clone(),
            app.state(),
            serde_json::json!({
                "virtualMicDevice": best.output,
                "virtualMicCapture": best.capture,
                "micPassthrough": mic.is_some(),
                "micDevice": mic,
            }),
        )?;
        Ok(best)
    })
    .await
}

/// Download and open VB-CABLE's own installer. True when a new virtual cable appeared.
#[tauri::command]
pub async fn install_virtual_cable(app: AppHandle) -> AppResult<bool> {
    blocking(app, |_, state| crate::cable::install(&state.paths.tmp_dir())).await
}

#[tauri::command]
pub fn get_route_checks(state: Cmd) -> Vec<VerifyResult> {
    state.route_checks()
}

// ── Settings & shortcuts ─────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: Cmd) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn update_settings(app: AppHandle, state: Cmd, patch: serde_json::Value) -> AppResult<Settings> {
    let prev = state.settings();
    let mut next = prev.merged(patch)?;
    shortcuts::check_globals(&state, &prev, &mut next)?;
    if next.remote_control && next.remote_token.is_empty() {
        next.remote_token = uuid::Uuid::new_v4().simple().to_string()[..16].to_owned();
    }
    // Before saving: a busy port must leave the setting off rather than claim it's on.
    if next.remote_control != prev.remote_control {
        crate::desktop::remote::configure(&app, next.remote_control).map_err(AppError::Invalid)?;
    }
    next.save(&state.db)?;
    state.replace_settings(next.clone());

    let v = &state.engine.volumes;
    v.master.set(next.master_volume);
    v.monitor.set(next.monitor_volume);
    v.mic.set(next.mic_volume);
    v.output.set(next.output_volume);
    crate::audio::mixer::MIC_DUCK.set(next.mic_duck_level());
    // Resolving the capture side enumerates devices, so only do it when routing actually changed
    // (not on every volume-slider tick).
    let routing = |s: &Settings| {
        let s = s.clone();
        (s.virtual_mic_device, s.virtual_mic_capture, s.monitor_device, s.mic_device, s.mic_passthrough, s.sample_rate, s.max_voices)
    };
    if routing(&prev) != routing(&next) {
        state.engine.configure(engine_config(&next));
    }
    let globals = |s: &Settings| {
        (s.stop_all_shortcut.clone(), s.replay_last_shortcut.clone(), s.overlay_shortcut.clone(), s.random_sound_shortcut.clone(), s.clip_shortcut.clone())
    };
    if globals(&next) != globals(&prev) {
        shortcuts::sync(&app);
    }
    if (next.clip_recording, next.clip_seconds) != (prev.clip_recording, prev.clip_seconds) {
        state.clips.configure(next.clip_recording, next.clip_seconds);
    }
    if next.ptt_key != prev.ptt_key {
        crate::desktop::ptt::set_held(None, false);
    }
    crate::desktop::apply_os_settings(&app, Some(&prev), &next).map_err(AppError::Invalid)?;
    Ok(next)
}

#[tauri::command]
pub fn set_sound_shortcut(app: AppHandle, state: Cmd, id: String, shortcut: Option<String>) -> AppResult<Sound> {
    let canon = match shortcut.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(sc) => Some(shortcuts::check_conflict(&state, &id, sc)?),
        None => None,
    };
    state.db.set_shortcut(&id, canon.as_deref())?;
    shortcuts::sync(&app);
    if let Some(c) = &canon
        && let Some(p) = shortcuts::problems(&state).into_iter().find(|p| &p.shortcut == c)
    {
        // Registration failed (another app owns it): don't keep a binding that silently does nothing.
        state.db.set_shortcut(&id, None)?;
        shortcuts::sync(&app);
        return Err(AppError::Shortcut(p.message));
    }
    state.db.get_sound(&id)
}

#[tauri::command]
pub fn get_shortcut_problems(state: Cmd) -> Vec<ShortcutProblem> {
    shortcuts::problems(&state)
}

// ── Downloads ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    installed: bool,
    managed: bool,
    version: Option<String>,
}

#[tauri::command]
pub async fn ytdlp_status(app: AppHandle) -> AppResult<ToolStatus> {
    blocking(app, |_, state| {
        let tools = state.paths.tools_dir();
        Ok(match ytdlp::locate(&tools) {
            Some(bin) => {
                ToolStatus { installed: true, managed: bin.starts_with(&tools), version: ytdlp::version(&bin) }
            }
            None => ToolStatus { installed: false, managed: false, version: None },
        })
    })
    .await
}

#[tauri::command]
pub async fn install_ytdlp(app: AppHandle) -> AppResult<ToolStatus> {
    blocking(app, |_, state| {
        let tools = state.paths.tools_dir();
        let bin = ytdlp::install(&tools)?;
        Ok(ToolStatus { installed: true, managed: true, version: ytdlp::version(&bin) })
    })
    .await
}

fn ytdlp_bin(state: &AppState) -> AppResult<PathBuf> {
    ytdlp::locate(&state.paths.tools_dir()).ok_or_else(|| {
        AppError::Download("The downloader component (yt-dlp) isn't installed yet. Install it from this page.".into())
    })
}

#[tauri::command]
pub async fn probe_url(app: AppHandle, url: String) -> AppResult<MediaInfo> {
    blocking(app, move |_, state| Downloader::probe(&ytdlp_bin(state)?, &url)).await
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    id: String,
    percent: f32,
}

#[tauri::command]
pub fn start_download(app: AppHandle, state: Cmd, info: MediaInfoInput) -> AppResult<DownloadRecord> {
    let bin = ytdlp_bin(&state)?;
    let (url, source) = sources::parse(&info.url)?;
    let id = uuid::Uuid::new_v4().to_string();
    let thumbnail = info.thumbnail.filter(|t| t.starts_with("https://"));
    state.db.insert_download(&NewDownload {
        id: &id,
        url: url.as_str(),
        source: source.label,
        title: info.title.as_deref(),
        thumbnail_url: thumbnail.as_deref(),
        duration_ms: info.duration_ms,
    })?;
    let record = state.db.get_download(&id)?;
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let dir = state.paths.tmp_dir().join("downloads");
        let progress_app = app.clone();
        let progress_id = id.clone();
        let result = state.downloader.download(&bin, &id, url.as_str(), &dir, move |percent| {
            let _ = progress_app.emit("download-progress", DownloadProgress { id: progress_id.clone(), percent });
        });
        let title = info.title.unwrap_or_else(|| "Your download".into());
        let outcome = match result {
            Ok(path) => match crate::audio::decode::decode_file(&path) {
                // Make sure it's playable now, not when the user clicks "Add".
                Ok(_) => {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                    crate::desktop::notify(&app, "Download ready", &format!("“{title}” is ready to add to your board."));
                    state.db.finish_download(&id, "ready", None, name.as_deref())
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&path);
                    state.db.finish_download(&id, "failed", Some(&e.to_string()), None)
                }
            },
            Err(e) => {
                let msg = e.to_string();
                let status = if msg == "Download cancelled." { "cancelled" } else { "failed" };
                if status == "failed" {
                    crate::desktop::notify(&app, "Download failed", &msg);
                }
                state.db.finish_download(&id, status, Some(&msg), None)
            }
        };
        if let Err(e) = outcome {
            log::error!("couldn't record download result: {e}");
        }
        let _ = app.emit("download-finished", &id);
    });
    Ok(record)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfoInput {
    url: String,
    title: Option<String>,
    thumbnail: Option<String>,
    duration_ms: Option<i64>,
}

#[tauri::command]
pub fn cancel_download(state: Cmd, id: String) -> bool {
    state.downloader.cancel(&id)
}

#[tauri::command]
pub fn list_downloads(state: Cmd) -> AppResult<Vec<DownloadRecord>> {
    state.db.list_downloads()
}

fn download_file(state: &AppState, id: &str) -> AppResult<(DownloadRecord, PathBuf)> {
    let rec = state.db.get_download(id)?;
    let file = rec
        .temp_file
        .clone()
        .filter(|_| rec.status == "ready")
        .ok_or_else(|| AppError::NotFound("This download isn't ready.".into()))?;
    let path = state.paths.tmp_dir().join("downloads").join(file);
    Ok((rec, path))
}

#[tauri::command]
pub async fn preview_download(app: AppHandle, id: String, start_ms: i64, end_ms: Option<i64>) -> AppResult<Option<u64>> {
    blocking(app, move |_, state| {
        let (rec, path) = download_file(state, &id)?;
        playback::preview(state, &path, rec.title.as_deref().unwrap_or("Download"), start_ms, end_ms, 1.0)
    })
    .await
}

#[tauri::command]
pub async fn import_download(
    app: AppHandle,
    id: String,
    name: String,
    category_id: Option<String>,
    trim_start_ms: i64,
    trim_end_ms: Option<i64>,
) -> AppResult<Sound> {
    blocking(app, move |_, state| {
        let (rec, path) = download_file(state, &id)?;
        let sound = library::import_file(
            &state.db,
            &state.paths,
            &path,
            ImportOptions {
                name: Some(name),
                category_id,
                volume: state.settings().default_sound_volume as f64,
                source_url: Some(rec.url),
                trim_start_ms,
                trim_end_ms,
            },
        )?;
        state.db.mark_download_imported(&id, &sound.id)?;
        state.cache.evict(&path);
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("couldn't remove temp download {}: {e}", path.display());
        }
        Ok(sound)
    })
    .await
}

#[tauri::command]
pub fn delete_download(state: Cmd, id: String) -> AppResult<()> {
    let rec = state.db.get_download(&id)?;
    state.downloader.cancel(&id);
    if let Some(f) = rec.temp_file {
        let _ = std::fs::remove_file(state.paths.tmp_dir().join("downloads").join(f));
    }
    state.db.delete_download(&id)
}

// ── Storage ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    data_dir: String,
    library_dir: String,
    covers_dir: String,
    sounds_bytes: u64,
    covers_bytes: u64,
    trash_bytes: u64,
    temp_bytes: u64,
    cache_bytes: u64,
    database_bytes: u64,
}

#[tauri::command]
pub async fn storage_info(app: AppHandle) -> AppResult<StorageInfo> {
    blocking(app, |_, state| {
        let p = &state.paths;
        Ok(StorageInfo {
            data_dir: p.root.to_string_lossy().into(),
            library_dir: p.library_dir().to_string_lossy().into(),
            covers_dir: p.covers_dir().to_string_lossy().into(),
            sounds_bytes: dir_size(&p.sounds_dir()),
            covers_bytes: dir_size(&p.covers_dir()),
            trash_bytes: dir_size(&p.trash_dir()),
            temp_bytes: dir_size(&p.tmp_dir()),
            cache_bytes: state.cache.bytes() as u64,
            database_bytes: [p.db_file(), p.root.join("blurt.db-wal")]
                .iter()
                .map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))
                .sum(),
        })
    })
    .await
}

#[tauri::command]
pub fn open_library_folder(app: AppHandle, state: Cmd) -> AppResult<()> {
    app.opener()
        .open_path(state.paths.library_dir().to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::Invalid(format!("Couldn't open the folder: {e}")))
}

#[tauri::command]
pub fn empty_trash(state: Cmd) -> AppResult<()> {
    let dir = state.paths.trash_dir();
    std::fs::remove_dir_all(&dir)?;
    std::fs::create_dir_all(&dir)?;
    Ok(())
}

/// Clears decoded audio from memory and temp files that no pending download needs.
#[tauri::command]
pub fn clear_cache(state: Cmd) -> AppResult<()> {
    state.cache.clear();
    crate::clean_temp(&state.db, &state.paths);
    Ok(())
}

#[tauri::command]
pub async fn backup_library(app: AppHandle, dest: String) -> AppResult<()> {
    blocking(app, move |_, state| crate::backup::create(&state.db, &state.paths, &PathBuf::from(dest))).await
}

/// Validates and stages the backup, then restarts so it's applied before the DB opens.
#[tauri::command]
pub async fn restore_library(app: AppHandle, src: String) -> AppResult<()> {
    blocking(app, move |app, state| {
        crate::backup::stage_restore(&state.paths, &PathBuf::from(src))?;
        app.request_restart();
        Ok(())
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    platform: &'static str,
    covers_dir: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: Cmd) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        platform: std::env::consts::OS,
        covers_dir: state.paths.covers_dir().to_string_lossy().into(),
    }
}
