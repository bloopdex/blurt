mod audio;
mod backup;
mod cable;
mod commands;
mod db;
mod desktop;
mod downloader;
mod error;
mod library;
mod paths;
mod playback;
mod settings;
mod state;
mod tts;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, RunEvent};

use audio::engine::{Engine, EngineEvent, Volumes};
use audio::mixer::AtomicF32;
use db::Db;
use paths::Paths;
use settings::Settings;
use state::{AppState, engine_config};

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just focuses the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| desktop::show_main(app)))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::Builder::new().arg("--autostart").build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(desktop::shortcuts::handle).build())
        .setup(|app| {
            setup(app.handle())?;
            Ok(())
        })
        .on_window_event(desktop::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_library,
            commands::import_files,
            commands::update_sound,
            commands::delete_sounds,
            commands::duplicate_sound,
            commands::set_sound_cover,
            commands::reorder_sounds,
            commands::move_sounds,
            commands::set_collection_membership,
            commands::create_group,
            commands::update_group,
            commands::delete_group,
            commands::reorder_groups,
            commands::play_sound,
            commands::preview_sound,
            commands::preview_file,
            commands::stop_voice,
            commands::stop_all,
            commands::replay_last,
            commands::save_clip,
            commands::text_to_speech,
            commands::tts_voices,
            commands::hide_overlay,
            commands::overlay_activity,
            commands::install_virtual_cable,
            commands::set_paused,
            commands::get_voices,
            commands::list_devices,
            commands::get_engine_status,
            commands::verify_virtual_mic,
            commands::get_route_checks,
            commands::auto_setup_virtual_mic,
            commands::get_settings,
            commands::update_settings,
            commands::set_sound_shortcut,
            commands::get_shortcut_problems,
            commands::ytdlp_status,
            commands::install_ytdlp,
            commands::probe_url,
            commands::start_download,
            commands::cancel_download,
            commands::list_downloads,
            commands::preview_download,
            commands::import_download,
            commands::delete_download,
            commands::storage_info,
            commands::open_library_folder,
            commands::empty_trash,
            commands::clear_cache,
            commands::backup_library,
            commands::restore_library,
            commands::app_info,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Blurt")
        .run(|app, event| {
            // macOS: clicking the Dock icon while the window is hidden should bring it back.
            #[cfg(target_os = "macos")]
            if let RunEvent::Reopen { .. } = event {
                desktop::show_main(app);
            }
            if let RunEvent::Exit = event
                && let Some(state) = app.try_state::<AppState>()
            {
                state.engine.stop_all(true);
                desktop::ptt::set_held(None, false);
            }
        });
}

fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let root = app.path().app_data_dir()?;
    std::fs::create_dir_all(&root)?;
    if backup::apply_pending(&root)? {
        log::info!("restored library from backup");
    }
    let paths = Paths::new(root)?;
    let db = Db::open(&paths.db_file())?;
    db.fail_interrupted_downloads()?;
    clean_temp(&db, &paths);
    let settings = Settings::load(&db)?;

    let volumes = Volumes {
        master: Arc::new(AtomicF32::new(settings.master_volume)),
        monitor: Arc::new(AtomicF32::new(settings.monitor_volume)),
        mic: Arc::new(AtomicF32::new(settings.mic_level())),
        output: Arc::new(AtomicF32::new(settings.output_volume)),
    };
    audio::mixer::MIC_DUCK.set(settings.mic_duck_level());
    let events = app.clone();
    let mic_was_open = AtomicBool::new(false);
    let engine = Engine::start(engine_config(&settings), volumes, move |ev| on_engine_event(&events, &mic_was_open, ev));

    app.manage(AppState::new(db, paths, engine, settings.clone()));
    app.state::<AppState>().clips.configure(settings.clip_recording, settings.clip_seconds);
    if settings.local_server()
        && let Err(e) = desktop::remote::configure(app, true)
    {
        log::warn!("{e}");
    }
    desktop::overlay::init(app);
    desktop::tray::create(app)?;
    desktop::shortcuts::sync(app);
    if let Err(e) = desktop::apply_os_settings(app, None, &settings) {
        log::warn!("{e}");
    }
    if !settings.launch_minimized {
        desktop::show_main(app);
    }
    Ok(())
}

fn on_engine_event(app: &AppHandle, mic_was_open: &AtomicBool, ev: EngineEvent) {
    match ev {
        EngineEvent::Voices(v) => {
            if let Some(state) = app.try_state::<AppState>() {
                let on_mic = v.iter().any(|x| x.on_mic && !x.paused);
                desktop::ptt::set_held(state.settings().ptt_key.as_deref(), on_mic);
            }
            let _ = app.emit("voices", v);
        }
        EngineEvent::DevicesChanged => {
            let _ = app.emit("devices-changed", ());
        }
        EngineEvent::Status(s) => {
            desktop::tray::update(app, &s);
            let open = s.virtual_mic.open;
            if mic_was_open.swap(open, Ordering::Relaxed) && !open && s.virtual_mic.device.is_some() {
                desktop::notify(app, "Virtual microphone disconnected", "Blurt will reconnect automatically when it's back.");
            }
            let _ = app.emit("engine-status", s);
        }
    }
}

/// Remove temp files that no pending ("ready") download refers to.
pub(crate) fn clean_temp(db: &Db, paths: &Paths) {
    let keep = db.ready_temp_files().unwrap_or_default();
    let busy: Vec<String> =
        db.list_downloads().unwrap_or_default().into_iter().filter(|d| d.status == "downloading").map(|d| d.id).collect();
    let dl = paths.tmp_dir().join("downloads");
    let Ok(entries) = std::fs::read_dir(&dl) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let in_use = keep.contains(&name) || busy.iter().any(|id| name.starts_with(id.as_str()));
        if !in_use
            && let Err(err) = std::fs::remove_file(e.path())
        {
            log::warn!("couldn't remove temp file {name}: {err}");
        }
    }
}
