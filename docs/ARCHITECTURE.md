# Blurt — Architecture

Blurt is a desktop soundboard whose one job is: **click a sound, and everyone listening to your microphone hears it.**

```
┌──────────────────────────── WebView (React + TypeScript) ────────────────────────────┐
│ Sidebar · Sound grid · Editor · Downloads · Settings · Onboarding · Player bar        │
│ Zustand stores ── src/lib/api.ts (typed invoke + Zod-parsed responses) ── events.ts   │
└───────────────────────────────────────────┬───────────────────────────────────────────┘
                                 Tauri IPC  │  commands ▲ events (voices, engine-status, …)
┌───────────────────────────────────────────┴─────────────────── Rust (src-tauri/src) ──┐
│ commands.rs ── playback.rs ── library.rs ── downloader/ ── backup.rs ── settings.rs   │
│      │              │             │              │                                    │
│      │        audio/engine.rs ◄── audio/cache.rs ◄── audio/decode.rs (Symphonia)      │
│      │         │  owns cpal streams on one thread                                     │
│      │         ├─ virtual-mic sink ──► "CABLE Input" ─(driver)─► "CABLE Output" mic   │
│      │         │      ▲ mic passthrough (audio/passthrough.rs, rtrb ring buffer)      │
│      │         └─ monitor sink ──► speakers / headphones (also previews)              │
│ db/ (SQLite, rusqlite, migrations)   desktop/ (tray, global shortcuts, lifecycle)     │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

## Layout

| Path | Responsibility |
|---|---|
| `src-tauri/src/audio/decode.rs` | Decode any supported file to interleaved stereo f32; waveform peaks; linear resampler |
| `src-tauri/src/audio/cache.rs` | Decoded buffers keyed by (file, device rate), 384 MB LRU budget |
| `src-tauri/src/audio/mixer.rs` | Real-time mixing: voices, trim window, fades, gain, soft limiter (lock-free, no steady-state allocation) |
| `src-tauri/src/audio/passthrough.rs` | Physical mic → virtual mic, with rate conversion and drift control |
| `src-tauri/src/audio/engine.rs` | Audio thread: opens/reopens devices, play/stop/pause, voice limit, status + progress events |
| `src-tauri/src/audio/devices.rs` | Device enumeration by direction, classification, format negotiation, sample-format conversion, change fingerprint |
| `src-tauri/src/audio/endpoints.rs` | OS endpoint metadata: data flow and owning adapter (Windows PnP via cfgmgr32) |
| `src-tauri/src/audio/routing.rs` | Loopback pairing, known-driver hints, and the signal test (received level, noise floor, quality) |
| `scripts/package.mjs` | Collects installers into `dist/` under stable names, verifies their format, writes checksums and `site/release.js` |
| `site/` | Static download page (`config.js` = where installers are hosted) |
| `src-tauri/src/playback.rs` | "Play sound X": behaviour modes, normalisation, honest routing warnings |
| `src-tauri/src/library.rs` | Import (validate → copy into library → insert), duplicate, delete-to-trash, covers |
| `src-tauri/src/downloader/` | URL validation/source detection, yt-dlp install/probe/download/cancel, error mapping |
| `src-tauri/src/db/` | Schema, migrations, repositories (sounds, groups, downloads, settings) |
| `src-tauri/src/desktop/` | Tray, global shortcuts, close-to-tray, login item, notifications |
| `src-tauri/src/backup.rs` | Zip backup with a `VACUUM INTO` snapshot; staged restore applied on next launch |
| `src/lib/api.ts` | The whole backend API, with Zod schemas that mirror the Rust structs |
| `src/store/` | Zustand stores: app (view, settings, engine, voices), library, downloads |
| `src/components`, `src/views` | UI |

## Architectural decisions

### ADR-001 — Virtual microphone via a virtual audio cable driver

**Problem.** Neither Windows nor macOS lets a user-mode application create a capture (microphone) endpoint. A soundboard that "plays into your mic" needs a driver that exposes a microphone.

**Options considered.**

1. *Write our own driver.* Windows: a kernel-mode WDM/AVStream virtual audio driver (or the SysVAD sample) must be signed through Microsoft's attestation/WHQL process with an EV certificate. macOS: an AudioServerPlugIn (user-space HAL plug-in in `/Library/Audio/Plug-Ins/HAL`) must be signed and notarized, and installation needs admin rights and a `coreaudiod` restart. Both are months of work plus paid certificates and per-OS-update maintenance.
2. *Windows APOs.* Audio Processing Objects attach to an existing endpoint's pipeline; they cannot create a new microphone, and third-party APO installation also requires a signed driver package. Rejected.
3. *Use an established virtual cable* — **chosen.** VB-CABLE (Windows, VB-Audio, donationware) and BlackHole (macOS, Existential Audio, GPL-3.0) are signed, widely deployed, and expose an output/input pair. Blurt plays into the output; other apps select the input as a microphone.

**Consequences.** One external install step, which the onboarding and Audio settings guide (with links, step lists, rescan, and a loopback test). Blurt does **not** bundle either driver: VB-CABLE's licence restricts redistribution and BlackHole is GPL-3.0. Detection is described in ADR-006: metadata and a signal test, with device names only as hints.

**No silent fallback.** If no virtual mic is configured or open, sounds still play on the monitor (so the user hears something), but every play returns a warning that the UI shows ("Only you can hear this"), the player bar and sidebar show *Mic offline / Set up mic*, cards show an *Only you* badge, and the tray shows the state.

### ADR-002 — Native audio engine in Rust (cpal), not Web Audio

The webview can only output to the default device and has no reliable device selection, multi-device output, or microphone capture into another device. Playback, mixing and routing are in Rust:

* **cpal** (WASAPI shared mode on Windows, Core Audio on macOS) for device I/O.
* **Symphonia** (pure Rust) for decoding: MP3, AAC-LC/ALAC in M4A/MP4, FLAC, Vorbis in OGG/WebM, WAV/AIFF. No ffmpeg dependency.
* One **engine thread** owns all streams (cpal streams aren't `Send`). Two sinks: *virtual mic* and *monitor*. Each sink's callback owns a `SinkRenderer`; new voices arrive over a channel, and stop/pause/progress use atomics, so the callback never takes a lock.
* Files are decoded once, resampled to each device's rate and cached; a voice is just an `Arc` to that buffer plus a cursor, so playing is instant after the first decode and many voices cost almost nothing.
* Output is soft-limited (transparent below −1 dBFS) to prevent clipping when sounds overlap; optional peak normalisation (to −3 dBFS, max +12 dB) evens out loudness; every stop has at least a 5 ms ramp to avoid clicks.
* **Mic passthrough** reads the physical mic into a lock-free SPSC ring (`rtrb`), resamples by the nominal rate ratio and trims backlog to ~30 ms to absorb clock drift, so users can pick the virtual mic in Discord and still talk.
* Device loss is reported by the stream error callback; the engine drops that sink, marks its voices finished, emits a status event, and retries every 2 s until the device is back.

### ADR-003 — SQLite with relative file names

`rusqlite` (bundled SQLite) with WAL and foreign keys. Rows store a file *name* (`<uuid>.<ext>`), never an absolute path; `paths.rs` resolves names under the app-data directory, so the library survives moves, upgrades and restores. Migrations are SQL files applied in a transaction and tracked with `PRAGMA user_version`.

Schema (`db/migrations/001_init.sql`):

* `groups` — categories (one per sound) and collections (many-to-many) share one table and one sidebar component (`kind` column). Name/colour/icon are validated.
* `sounds` — metadata, non-destructive trim, per-sound volume and playback mode, favourite/pin, play count, `last_played_at`, manual `position`, unique `shortcut`, waveform peaks (96 bytes), `source_url`. `category_id` is `ON DELETE SET NULL` so deleting a category never deletes sounds.
* `collection_items` — `ON DELETE CASCADE` on both sides.
* `downloads` — download history with `downloading/ready/imported/failed/cancelled`.
* `settings` — one JSON document; missing fields fall back to defaults so older data loads after upgrades.

Favourites, pinning and "recently played" are columns rather than tables: one row per sound, no joins, and the indexed `last_played_at` serves the recent view. A separate playback-history table was not needed for any feature.

### ADR-004 — Downloads through yt-dlp, fetched on demand

yt-dlp is the maintained extractor for YouTube, TikTok, Instagram and ~1,800 other sites. It's downloaded on first use from the official GitHub release into the app's `tools/` folder (or taken from `PATH`), so it can be updated independently of Blurt — sites change weekly.

* URLs are validated in Rust (`http(s)` only, real host) and passed after `--` so they can't be read as options.
* Format selection prefers formats Symphonia decodes without ffmpeg: `bestaudio[ext=m4a]` (AAC) → mp3 → mp4 with AAC. `--fixup never` avoids needing ffmpeg.
* No cookies, no sign-in, no DRM circumvention. Private, age-restricted, geo-blocked, DRM, live and playlist links fail with specific messages (`ytdlp::friendly_error`).
* Downloading is separate from playback: it produces a temp file that goes through the normal import path after the user reviews it (name, category, trim, preview).

### ADR-005 — Desktop integration through Tauri plugins

`tray-icon` (tray + menu showing routing status), `global-shortcut` (OS-wide hotkeys, press *and* release for hold-to-play), `autostart` (Windows `Run` key / macOS LaunchAgent), `single-instance` (a second launch focuses the first), `notification` (only while the window is hidden), `dialog`, `opener`, `log` (rotating log file in the OS log dir). Closing the window hides it when "close to tray" is on; the engine keeps running.

Shortcuts are stored canonically (`control+alt+Digit1`) so different spellings can't collide; conflicts are checked against other sounds and the stop-all shortcut, bare keys are refused (they'd hijack typing system-wide), and a binding another app already owns is rolled back with an error instead of being saved dead.

### ADR-006 — Classify audio devices by OS metadata and prove routes with a signal test

**Problem.** Version 0.1 recognised virtual cables by matching device names against a list. Selecting `Speakers (NVIDIA Broadcast)` showed "isn't a recognised virtual cable", which was right for the wrong reason: the name wasn't on the list. Names can't tell you what a device does. A name like "Microphone (X)" on a playback endpoint, or "Virtual" on hardware, would be misclassified.

**What NVIDIA Broadcast actually is** (measured on the Windows test machine, with Broadcast running): one root-enumerated software adapter (`ROOT\UNNAMED_DEVICE\0000`) that exposes a **render** endpoint, `Speakers (NVIDIA Broadcast)`, and a **capture** endpoint, `Microphone (NVIDIA Broadcast)`. A 1 kHz tone played into the speakers endpoint **never appears** on the microphone endpoint (three runs, no tone above the noise floor). The speakers endpoint processes audio you *hear* and forwards it to your real output. The microphone endpoint carries your physical mic after noise removal. There's no supported way to inject external audio into its microphone pipeline.

**Decision.** Evidence in order of strength:

1. **Direction comes from the OS audio API:** cpal's render vs capture enumeration, cross-checked against the MMDevice instance id (`{0.0.0…}` = render, `{0.0.1…}` = capture). A device is only ever offered for the role its direction allows. Picking a recording device as the playback target, or the reverse, is reported as *Incorrect device direction*.
2. **Adapter metadata** (`endpoints.rs`, Windows): each endpoint's `DEVPKEY_Device_Parent`. `ROOT\…` means a software driver; `HDAUDIO\`, `USB\` and `BTHENUM\` mean hardware. A render endpoint and a capture endpoint on the **same software adapter** are a *candidate* loopback pair. Hardware outputs are never paired.
3. **Name hints** (`KNOWN_LOOPBACKS`) mark drivers known to loop: VB-CABLE, Voicemeeter, BlackHole, Loopback, VAC and Steam. They decide grouping and labels, never direction or pairing on Windows. NVIDIA Broadcast is deliberately not in the list.
4. **The signal test** is the only proof. It plays a −12 dBFS, 1 kHz tone for 0.8 s into the playback device while capturing the recording device. Goertzel detection gives the received level, and the pre-tone window gives the noise floor. *Signal received* (≥ 12 dB above the floor and above −54 dBFS) is reported separately from *quality OK* (loss ≤ 18 dB and SNR ≥ 20 dB). Results are stored per (playback, recording) pair. A failed test turns the status into *Mic not reached*, and every play warns.

The user picks the playback side and the recording side separately. The recording side defaults to the metadata pair and can be overridden. Mic passthrough refuses the configured recording side as its source, to prevent a feedback loop. Any other microphone works, including `Microphone (NVIDIA Broadcast)`, which is the supported way to combine Broadcast with Blurt.

**Device changes.** The engine fingerprints the device set and the system defaults every 5 s. On a change it:
- emits `devices-changed`, so the UI refreshes;
- retries any configured device that's missing;
- moves a monitor set to "system default" over to the new default.

Stream errors (unplug, audio service restart) drop the sink immediately and retry every 2 s. Device choices are stored by name and kept while a device is absent. The virtual-mic sink only ever opens the named device, never a fallback.

**macOS.** `endpoints::list()` returns nothing there, so pairing falls back to known-loopback names (BlackHole uses the same name in both directions). The Core Audio equivalent of the adapter check is `kAudioDevicePropertyTransportType == kAudioDeviceTransportTypeVirtual`. It's left as a marked shortcut until the code can be tested on a Mac.

### ADR-007 — Distribution: NSIS per-machine installer, stable asset names, draft releases

* **Installer.** Tauri's NSIS template with `installMode: perMachine`:
  * installs into Program Files and writes the HKLM uninstall entry (name, icon, version, publisher, location);
  * always adds a Start Menu shortcut, and offers a desktop shortcut on the finish page (created automatically in silent mode);
  * ships with the Blurt icon and installer artwork.

  The uninstaller deletes `%APPDATA%\app.blurt.desktop` only if the user ticks the box (unticked by default), and never in update mode. User data lives outside the install folder, so upgrades keep it. The MSI (WiX) is offered for managed deployment.
* **dist/.** Vite now builds into `build/`, which is what gets embedded in the binary; `dist/` holds only release downloads. `scripts/package.mjs`:
  * copies the installers to `dist/Blurt-Setup.exe`, `.msi` and `Blurt-macOS-{arm64,x64}.dmg`;
  * rejects any file whose magic numbers are wrong (MZ + PE + NSIS marker, OLE, UDIF `koly`);
  * writes `SHA256SUMS.txt` and `site/release.js`.
* **Releases.** `release.yml` runs on `v*` tags. It fails if the tag doesn't match the app version, builds on native Windows and macOS runners, packages, checksums, and opens a **draft** release with notes from `CHANGELOG.md`. Version-less asset names make `releases/latest/download/Blurt-Setup.exe` a permanent link for the download page.
* **Updates.** Not shipped. The signed-update design is in `docs/UPDATES.md`.

## Data flow: playing a sound

1. Card click → `api.playSound(id)` → `play_sound` (async, off the UI thread).
2. `playback::play_sound` loads the row, checks the file exists, reads which sinks are open and at what rate.
3. `AudioCache::get` returns buffers for those rates (decode + resample on first use).
4. `Engine::play` sends a `PlayRequest`; the engine applies the behaviour (restart/toggle/overlap, stop-others, voice limit) and hands a `Voice` to each sink.
5. The engine emits `voices` every 100 ms while anything plays (and is idle otherwise); the grid, player bar and queue render progress from that.
6. The command returns `{ voiceId, onMic, warning }`; a warning becomes a toast.

## Security notes

* CSP restricts scripts to the app bundle; images allow `https:` (download thumbnails) and the asset protocol, which is scoped to `$APPDATA/library/covers/**`.
* Capabilities grant the webview only core IPC, `dialog:allow-open/save` and `opener:default`. Everything else is a Rust command with validation (`deny_unknown_fields` on patches, length/range checks, kind checks for groups).
* Backup restore rejects foreign archives and uses `enclosed_name()` against zip-slip; only `blurt.db` and `library/{sounds,covers}` are extracted.
