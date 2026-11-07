# Implementation status

_Last updated: 2026-11-05._ Legend: ✅ done and verified · 🟡 done, verified only partly (see note) · ⛔ not implemented.

**Verification environment:** Windows 11 Pro 26200, x86_64, Realtek + NVIDIA HDMI audio, *Steam Streaming Microphone* virtual cable, **NVIDIA Broadcast** (running). **Nothing has been run on macOS yet.**

## v0.2.0 — device detection & routing

| Feature | Status | Evidence |
|---|---|---|
| Direction from the OS (render vs capture), never from names | ✅ Windows | `real_devices_are_classified_by_direction` hardware test: 6 playback + 3 recording endpoints, none in the wrong list; wrong-direction choices rejected in the app ("is a recording device…") |
| Software vs hardware from PnP metadata (`DEVPKEY_Device_Parent`, `ROOT\` = software) | ✅ Windows | Every endpoint on the machine has adapter metadata; Realtek/NVIDIA HDMI = hardware, NVIDIA Broadcast/Steam = software |
| Loopback candidate = render + capture on the same software adapter | ✅ Windows | NVIDIA Broadcast and Steam paired as candidates; hardware never paired; Steam Streaming *Speakers* (no capture side) unpaired |
| NVIDIA Broadcast speakers → NVIDIA Broadcast mic | ✅ **measured: does not loop** | 3 signal tests, no tone above the noise floor. The app shows *Audio test failed — no signal*, the pill says *Mic not reached*, plays warn |
| NVIDIA Broadcast mic as passthrough source (supported combination) | 🟡 | Passthrough opens it (48 kHz, live) and mixes into the cable. Not yet listened to on a real call; the cable here (Steam) is −33 dB attenuated |
| Signal test: received vs quality, noise floor, stored per pair | ✅ | Unit-tested on synthetic captures; on hardware: Steam = received/poor (−37…−45 dBFS), NVIDIA = not received |
| Separate playback / recording selection, status checklist, combined output volume | ✅ | E2E over CDP against real devices; combined gain unit-tested in the mixer |
| Device-change detection (5 s fingerprint → refresh, reconnect, default-follow) | 🟡 | Implemented; no device was physically plugged or unplugged, and the default device wasn't switched during testing |
| Fast device listing via the MMDevice API, with the same names apps see | ✅ Windows | `fast_device_names_match_cpal`; listing 2,158 → 12 ms; mic ready at the same time as the window |
| Set up automatically (tests every software pair, fixes level, enables passthrough) | ✅ Windows | E2E from the user's real broken config (Realtek speakers → Realtek mic): picked Steam, raised its input level 0 → 21.7 dB, −12…−21 dBFS received for −12 sent, pill *Mic live*, sound on mic |
| Fix level (raise the virtual mic's Windows input level, 6 dB headroom) | ✅ Windows | Steam Streaming Microphone loses 26–32 dB, varying between runs; after the fix, quality checks pass |
| Real speakers/mics flagged (never shown as working) | ✅ | E2E: checklist *These are real speakers…*, pill *Mic not reached*, play warning |
| Passthrough from every mic on the PC | 🟡 | `passthrough_from_every_microphone`: NVIDIA Broadcast mic → cable carried the acoustic test beep. Realtek mic → cable carried live room audio, but the beep wasn't distinguishable from it |
| macOS metadata (Core Audio transport type) | ⛔ | Falls back to known-loopback names (BlackHole); marked `shortcut:` in `endpoints.rs` |

## Core: virtual microphone

| Feature | Status | Evidence |
|---|---|---|
| Engine → virtual cable → cable's microphone side | ✅ Windows | `engine_plays_into_virtual_microphone` hardware test: 660 Hz test sound played through the engine and captured from *Microphone (Steam Streaming Microphone)* ~100 dB above the digital noise floor |
| In-app "Test virtual microphone" (1 kHz loopback + Goertzel detector) | ✅ Windows | Ran in the app; correctly reported the Steam cable as "faint (−39 dBFS)" |
| Clicking a card plays into the virtual mic | ✅ Windows | E2E over CDP: `get_voices` → `onMic: true` |
| Virtual cable detection & pairing | 🟡 | By OS metadata (see v0.2.0 table); name hints unit-tested for VB-CABLE, Voicemeeter, BlackHole, Loopback, VAC, Steam. Only the Steam cable exists on the test machine |
| VB-CABLE end to end | 🟡 | Not installed on the test machine (it's a third-party driver needing admin and a reboot). Same code path as the verified Steam cable. |
| BlackHole on macOS | ⛔ untested | Code path is cpal/Core Audio; needs a Mac |
| Microphone passthrough (real mic mixed into the virtual mic) | 🟡 | Ring buffer/resampler/drift logic unit-tested; not yet listened to through a live call |
| Device disconnect → status event, toast, tray update, auto-reconnect | 🟡 | Implemented; not exercised by physically unplugging a device |
| No silent fallback | ✅ | Plays without a virtual mic return a warning (toast, "Only you" badge, "Set up mic" pill) |

**Observation:** Steam Streaming Microphone attenuates ~30 dB, so it works but is quiet. VB-CABLE or BlackHole are what we recommend.

## Soundboard

| Feature | Status |
|---|---|
| Import one/many files (picker or drag-and-drop onto the window), copied into the managed library, unsupported files reported per file | ✅ (picker/drag UI not automated; the backend import was run with real Windows WAVs and a rejected `.ini`) |
| MP3 / WAV / OGG / FLAC / M4A (AAC-LC, ALAC) / AIFF / WebM-Vorbis | 🟡 WAV and M4A verified with real files; others rely on Symphonia's decoders |
| WebM/Opus, WMA, HE-AAC | ⛔ refused with an explanatory message |
| Grid with waveform art or cover, duration, shortcut, favourite, live progress | ✅ |
| Density (compact/comfortable/large), sort (custom/name/recent/plays/duration), search | ✅ |
| Drag to reorder; drag onto a category/collection in the sidebar | 🟡 implemented with dnd-kit pointer sensors; not automated |
| Right-click menu (play, preview, edit, favourite, pin, move, collections, cover, duplicate, delete) | ✅ |
| Sound editor: name, description, category, volume 0–200 %, playback mode, non-destructive trim on waveform with preview, cover, file info, shortcut | ✅ |
| Favourites, pinning, recently played, play counts, downloaded view | ✅ |
| Categories & collections: create, rename, icon, colour, reorder (drag), delete (sounds kept) | ✅ (reorder by drag not automated) |
| Delete to trash with optional confirmation; empty trash | ✅ |
| Bottom player: now playing, progress, pause/resume all, stop all, per-sound queue, mic & monitor volume, routing pill | ✅ |
| Multiple simultaneous sounds, voice limit, restart/toggle/overlap/hold modes, stop-previous, fades, normalisation, limiter | ✅ unit-tested mixer + engine paths |

## Downloads

| Feature | Status |
|---|---|
| One-click yt-dlp install from the official release; update | ✅ installed yt-dlp 2026.10.26 |
| URL validation, source badge, metadata preview (title, thumbnail, uploader, duration) | ✅ YouTube verified |
| Download with progress and cancel; history; retry | 🟡 download + history verified; cancel not exercised |
| Review step: rename, category, start/end trim, preview, add | ✅ (imported "Me at the zoo" trimmed 1–6 s, played on mic) |
| TikTok / Instagram | 🟡 not tested; same yt-dlp path |
| Error mapping (private, age, region, DRM, removed, unsupported, rate limit, network) | ✅ unit-tested; "unavailable" and bad-domain cases verified live |

## Desktop integration

| Feature | Status |
|---|---|
| Global shortcuts per sound (works while another app is focused), stop-all shortcut, conflict detection, bare-key refusal, hold-to-play | ✅ Ctrl+Alt+1 fired from Notepad and played into the mic; hold-to-play not exercised |
| Tray icon & menu (open, pause/resume, stop all, routing status, quit) | 🟡 implemented; menu not clicked in automation |
| Close to tray; restore via second launch (single instance) | ✅ |
| Start with Windows (registry `Run` key, `--autostart`) | ✅ entry added and removed |
| Launch minimised, notifications while hidden | 🟡 implemented, not exercised |
| macOS: menu bar tray, Dock visibility, LaunchAgent, Dock reopen | ⛔ untested (code compiled only for Windows) |

## Settings, storage, polish

| Feature | Status |
|---|---|
| General / Audio / Playback / Shortcuts / Storage / About tabs | ✅ |
| Dark (default) / light / system theme | ✅ |
| Storage usage, open library folder, empty trash, clear cache | ✅ |
| Backup to .zip and restore (staged, applied on restart) | 🟡 round trip unit-tested; not done through the UI |
| First-launch onboarding (welcome, one-click *Set up my microphone*, first sounds, shortcuts; skippable; manual devices under *Choose devices myself*) | 🟡 typechecked; the button was not clicked on this PC (it plays a test tone, and the user was in a Discord call) |
| Replay last sound shortcut (default Ctrl+Alt+,) | ✅ real keypress replayed the last sound into CABLE Input |
| In-game overlay (default Ctrl+Alt+B; pinned first, keys 1–9/0, auto-close after 4 s idle, playing highlight, Replay, Stop) | 🟡 never takes focus (SW_SHOWNOACTIVATE + top-most), keys are global only while open, opens on the cursor's monitor. User reported Esc swallowed by Valorant (hence auto-close). Not yet confirmed over Valorant |
| Random sound shortcut (default Ctrl+Alt+R) | 🟡 builds; not pressed here |
| Duck passthrough mic while a sound plays (−12 dB, ~20 ms smoothing) | ✅ mixer unit test |
| Auto push-to-talk key while sounds play (SendInput scan codes / mouse buttons) | ✅ live: F24 held for the 1.5 s sound and released after; unsupported keys refused. Not tried in Discord/Valorant |
| Instant clips (WASAPI loopback ring buffer, 5–120 s, Ctrl+Alt+C, Clip now) | ✅ live: 5.0 s clip at 48 kHz with real audio; WAV round-trip unit test; empty buffer refused |
| Remote control HTTP API (127.0.0.1:7270, token) | ✅ live: 401 without/with wrong token, /sounds /play /random /stop work, 404 unknown, 405 POST, port closes when off and rebinds when on. Not tried with a real Stream Deck |
| Text to speech (SAPI via PowerShell, voices list) | ✅ live: 3.4 s WAV sound; empty text refused; shell metacharacters stay text (passed via environment) |
| One-click VB-CABLE install (download from vb-audio.com, Authenticode signer check, VB-Audio's own setup elevated, then auto setup; finishes after restart) | 🟡 download, unzip and signature (Valid, BUREL VINCENT) verified by hand; the installer step was not run here (VB-CABLE is already installed) |
| Language selection | ⛔ English only |
| Check for updates | ⛔ needs a release endpoint and updater signing keys |
| Audio-library relocation | ⛔ location is shown and openable, not movable |

## Build & distribution

| Item | Status |
|---|---|
| `dist/Blurt-Setup.exe` (v0.2.0, per-machine NSIS, 3.4 MB) + `dist/Blurt-Setup.msi` (4.6 MB) + `SHA256SUMS.txt` | ✅ built; format verified (MZ/PE + NSIS marker, OLE). `INSTALLMODE perMachine`, `RequestExecutionLevel admin` confirmed in the generated script |
| Per-machine install into Program Files | ✅ 0.2.1 installed elevated (`/S`) over the user's own 0.2.0 install: exit 0, HKLM uninstall entry 0.2.1, installed app launched, reports 0.2.1, plays into the virtual mic |
| Install → launch → upgrade → launch → uninstall lifecycle | ✅ 20/20 checks, using per-user builds from the same template (no elevation needed): 0.1.0 install (exe, HKCU uninstall entry, Start Menu + desktop shortcuts), app launches and reports 0.1.0, in-place upgrade to 0.2.0 (single entry, version/publisher/icon/uninstaller), app launches and reports 0.2.0 with the same 7 sounds and settings, silent uninstall removes program/entry/shortcuts and **keeps user data**; sound files byte-identical to the pre-test backup |
| Release exe idle cost | ~37 MB, ~0.6 % of one core (two open audio streams); WebView2 adds ~345 MB across its shared processes |
| Landing page (`site/`): hero, features, gamer/creator sections, setup, changelog, FAQ; real screenshots in `site/assets/` from a demo profile; self-hosted fonts, no third-party requests | ✅ rendered in Edge at 1440 px and 390 px over HTTP with no console errors; download buttons enable only from `release.js` and resolve to the real `dist/Blurt-Setup.exe`; macOS shown as coming soon |
| macOS DMG (Apple Silicon + Intel) | ⛔ not built locally; CI matrix defined, untested |
| GitHub Actions: CI (`build.yml`), releases (`release.yml`: tag check, native builds, format check, checksums, draft release) | 🟡 written, never run (no Git repository or remote yet; see `docs/RELEASING.md`) |
| Automatic updates | ⛔ by design until signed (`docs/UPDATES.md`) |
| Code signing (Windows Authenticode, Apple notarization) | ⛔ documented, needs certificates |

## Known limitations

* Whole-file decode: clips are capped at 15 minutes (memory). Streaming decode would lift this.
* Linear-interpolation resampling: fine for voice chat, not audiophile-grade.
* Windows device identity is the friendly name; renaming a device in Sound settings means picking it again.
* Device changes are noticed by polling every 5 s (stream errors are immediate), not by OS change notifications.
* NVIDIA Broadcast can't be the playback target: its speaker device doesn't feed its microphone. Use a virtual cable plus passthrough of its microphone.
* Hold-to-play: a very quick tap can arrive before the voice is registered, so the sound plays to the end.
* Keyboard reordering of cards isn't supported (Enter/Space play the sound); use sort or drag.
* The download segment picker uses typed start/end times; the waveform trim is in the sound editor after import.
