//! Remote control for Stream Deck ("Website" action, GET in background), Touch Portal, Streamer.bot,
//! OBS scripts…: plain HTTP GET on 127.0.0.1 only, and every request needs the token from settings
//! so a web page you visit can't fire sounds.
//!
//! GET /play?name=Airhorn&token=…   (or id=…)   /random   /replay   /stop   /clip   /overlay   /sounds

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

pub const PORT: u16 = 7270;

static RUNNING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

pub fn configure(app: &AppHandle, on: bool) -> Result<(), String> {
    let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(flag) = running.take() {
        flag.store(true, Ordering::Relaxed);
    }
    if !on {
        return Ok(());
    }
    // The previous listener closes within one poll interval.
    let mut bound = TcpListener::bind(("127.0.0.1", PORT));
    for _ in 0..10 {
        if bound.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(60));
        bound = TcpListener::bind(("127.0.0.1", PORT));
    }
    let listener = bound.map_err(|e| format!("Remote control needs port {PORT}, but another app is using it ({e})."))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    *running = Some(stop.clone());
    let app = app.clone();
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let app = app.clone();
                    std::thread::spawn(move || serve(&app, stream));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    });
    Ok(())
}

fn serve(app: &AppHandle, mut stream: TcpStream) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buf = [0u8; 4096];
    let mut len = 0;
    while len < buf.len() {
        match stream.read(&mut buf[len..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => len += n,
        }
        if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8_lossy(&buf[..len]);
    let target = head.lines().next().and_then(|l| l.strip_prefix("GET ")).and_then(|l| l.split(' ').next());
    let (status, body) = match target {
        Some(t) => route(app, t),
        None => (405, json!({ "ok": false, "error": "Use GET." })),
    };
    let body = body.to_string();
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Bad Request",
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn route(app: &AppHandle, target: &str) -> (u16, Value) {
    let Ok(url) = url::Url::parse(&format!("http://localhost{target}")) else {
        return (400, json!({ "ok": false, "error": "Bad request." }));
    };
    let param = |k: &str| url.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.into_owned());
    let state = app.state::<AppState>();
    let token = state.settings().remote_token;
    if token.is_empty() || param("token").as_deref() != Some(token.as_str()) {
        return (401, json!({ "ok": false, "error": "Missing or wrong token. Copy the link from Blurt → Settings → Stream & game." }));
    }
    let played = |r: crate::error::AppResult<crate::playback::PlayOutcome>| match r {
        Ok(o) => (200, json!({ "ok": true, "warning": o.warning })),
        Err(e) => (400, json!({ "ok": false, "error": e.to_string() })),
    };
    match url.path() {
        "/play" => {
            let sounds = state.db.list_sounds().unwrap_or_default();
            let found = match (param("id"), param("name")) {
                (Some(id), _) => sounds.iter().find(|s| s.id == id),
                (None, Some(n)) => find_by_name(&sounds, &n),
                _ => return (400, json!({ "ok": false, "error": "Add ?name=… or ?id=…" })),
            };
            match found {
                Some(s) => played(crate::playback::play_sound(app, &state, &s.id, None)),
                None => (404, json!({ "ok": false, "error": "No sound with that name." })),
            }
        }
        "/random" => played(crate::playback::play_random(app, &state)),
        "/replay" => played(crate::playback::replay_last(app, &state)),
        "/stop" => {
            state.engine.stop_all(false);
            (200, json!({ "ok": true }))
        }
        "/clip" => match super::save_clip(app) {
            Ok(s) => (200, json!({ "ok": true, "name": s.name })),
            Err(e) => (400, json!({ "ok": false, "error": e.to_string() })),
        },
        "/overlay" => {
            super::overlay::toggle(app);
            (200, json!({ "ok": true }))
        }
        "/sounds" => {
            let list: Vec<Value> =
                state.db.list_sounds().unwrap_or_default().iter().map(|s| json!({ "id": s.id, "name": s.name, "shortcut": s.shortcut })).collect();
            (200, json!({ "ok": true, "sounds": list }))
        }
        _ => (404, json!({ "ok": false, "error": "Unknown action. Use /play, /random, /replay, /stop, /clip, /overlay or /sounds." })),
    }
}

/// Exact name (ignoring case) wins; otherwise a single partial match.
fn find_by_name<'a>(sounds: &'a [crate::db::sounds::Sound], name: &str) -> Option<&'a crate::db::sounds::Sound> {
    let n = name.to_lowercase();
    sounds.iter().find(|s| s.name.to_lowercase() == n).or_else(|| {
        let mut partial = sounds.iter().filter(|s| s.name.to_lowercase().contains(&n));
        let first = partial.next();
        if partial.next().is_some() { None } else { first }
    })
}
