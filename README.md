<p align="center"><img src="design/logo.svg" width="88" alt="Blurt logo"></p>

<h1 align="center">Blurt</h1>
<p align="center"><b>Say it with sound.</b> A desktop soundboard that plays straight into your microphone —<br>so everyone on Discord, Zoom, Teams, Meet or in-game voice chat hears it.</p>

<p align="center"><a href="https://bloopdex.github.io/blurt/"><b>Website</b></a> · <a href="https://github.com/bloopdex/blurt/releases/latest/download/Blurt-Setup.exe"><b>Download for Windows</b></a> · <a href="https://github.com/bloopdex/blurt/releases">Releases</a> · <a href="https://github.com/bloopdex/blurt/actions">CI</a></p>

---

* **Into your mic, for real.** Sounds go into a virtual microphone; a built-in loopback test proves the signal arrives. If it can't reach the mic, Blurt tells you — it never silently plays only on your speakers.
* **Keep talking.** Optional mic passthrough mixes your own voice into the virtual mic, so you never switch inputs.
* **Fast.** Native Rust audio engine, multiple sounds at once, global hotkeys that work while you're in a game, tray-resident.
* **Organised.** Categories, collections, favourites, pins, recently played, play counts, search, drag-and-drop.
* **Get sounds anywhere.** Import MP3/WAV/OGG/FLAC/M4A/AIFF, or paste a YouTube/TikTok/Instagram link, preview, trim and add.

## Install

Download **[Blurt-Setup.exe](https://github.com/bloopdex/blurt/releases/latest/download/Blurt-Setup.exe)** from the [latest release](https://github.com/bloopdex/blurt/releases/latest), or visit [the website](https://bloopdex.github.io/blurt/). Every release is built by GitHub Actions from its tag and lists SHA-256 checksums. To build it yourself, run `pnpm release:win` (see [`docs/RELEASING.md`](docs/RELEASING.md)).

* **Windows 10/11:** `Blurt-Setup.exe` installs for all users into *Program Files* (asks for admin), adds a Start Menu shortcut and an optional desktop shortcut, and registers an uninstaller under *Settings → Apps*. Installing a newer version upgrades in place. Your sounds and settings are kept on upgrade **and** on uninstall, unless you tick *Delete the application data* in the uninstaller. The build is unsigned, so SmartScreen shows *More info → Run anyway*. `Blurt-Setup.msi` is for managed deployment.
* **macOS 11+:** `Blurt-macOS-arm64.dmg` (Apple Silicon) or `Blurt-macOS-x64.dmg` (Intel), built by CI. **Not yet tested on a Mac.**

`site/` is a static download page. Point `site/config.js` at your release URL; until a real installer is listed in `site/release.js`, the buttons stay disabled.

## Virtual microphone setup

Apps can't create microphones on their own, so Blurt plays into a free **virtual audio cable**. Its other end shows up as a microphone in every app. Blurt's onboarding walks you through this, and **Settings → Audio** has the same tools.

**Settings → Audio** has two selectors, because a cable has two ends:

* **Blurt plays into**: playback devices only, grouped as *Virtual cables*, *Software devices — test before use* and *Speakers & other playback devices*.
* **Other apps use this microphone**: recording devices only. *Automatic* picks the recording endpoint on the same software driver, using the OS device tree, not the name.

Blurt reads each device's direction (playback or recording) and driver from the OS. Names are only used as hints for known cables. A recording device can't be chosen as the output, and the reverse is also refused. The checklist under the selectors shows four separate states: **Playback** (connected / unavailable / wrong direction), **Virtual microphone** (found / unverified), **Signal** (the test tone arrived or not) and **Quality** (level and noise). Only **Test virtual microphone** proves that a pair works. Results are remembered per device pair, and a failed pair is flagged when you play.

Devices are re-checked every 5 seconds. If the chosen device disappears, Blurt says so and reconnects when it's back. It never switches to another device on its own.

**Set up automatically** does all of this for you: it tests every virtual device on the PC, picks one that really reaches a microphone, raises its input level if it's too quiet (**Fix level**, 6 dB below what was sent), and mixes your real mic in. Real speakers (e.g. *Speakers (Realtek)*) are flagged: sound sent there goes into the room, not into a microphone.

**Levels:** *Soundboard* (your sounds), *Microphone* (your voice, with passthrough on), *Combined output* (everything sent to the virtual mic).

### Stream & game features

*Settings → Stream & game*: **auto push-to-talk** (Blurt holds your PTT key or mouse button while a sound plays), **instant clips** (save the last 5–120 s of what you hear with Ctrl+Alt+C), and **Stream Deck / Touch Portal / Streamer.bot control**. For remote control, turn it on, copy a link such as `http://127.0.0.1:7270/play?name=Airhorn&token=…`, and use Stream Deck’s *Website* action with *GET request in background*. The **Speak** button in the library turns typed text into a sound and plays it into the mic.

### Windows — VB-CABLE

On first launch, **Set up my microphone** does all of this: if no working virtual cable is found, Blurt downloads VB-CABLE from vb-audio.com, checks it is signed by VB-Audio, and opens VB-Audio's installer. Click **Install Driver**, approve the Windows prompt, and Blurt configures the rest (after a restart if Windows needs one). VB-CABLE's licence doesn't allow bundling it inside Blurt's own installer, so it can't be installed silently with Blurt.

To do it by hand:

1. Download VB-CABLE from <https://vb-audio.com/Cable/> (donationware) and unzip it.
2. Right-click `VBCABLE_Setup_x64.exe` → **Run as administrator** → **Install Driver**. Reboot if asked.
3. In Blurt: **Settings → Audio → Rescan**, then set *Play sounds into* to **CABLE Input (VB-Audio Virtual Cable)**.
4. Click **Test virtual microphone**. You should see *Signal received*.
5. In Discord → *Voice & Video* → *Input Device*: **CABLE Output (VB-Audio Virtual Cable)**. Same idea in Zoom, Teams, Meet or your game.
6. Turn on **Mix in my microphone** in Blurt so your voice goes through too.

Voicemeeter, Virtual Audio Cable and Steam Streaming Microphone are also detected. Steam's driver works but is about 30 dB quieter.

### NVIDIA Broadcast

NVIDIA Broadcast is **not** a virtual cable. Its *Speakers (NVIDIA Broadcast)* device cleans up what *you* hear. It does not feed *Microphone (NVIDIA Broadcast)*: in testing, no test tone reached it. To use both:

1. Blurt plays into a real virtual cable (*CABLE Input*).
2. Turn on **Mix in my microphone** and choose **Microphone (NVIDIA Broadcast)**, so your noise-cancelled voice is mixed in.
3. In Discord, set the input device to the cable's recording side (*CABLE Output*).

If you pick NVIDIA Broadcast's speakers anyway, the test reports *No signal* and the pill says *Mic not reached*.

### macOS — BlackHole

1. Install **BlackHole 2ch** from <https://existential.audio/blackhole/> or `brew install blackhole-2ch`. Restart if it doesn't appear.
2. In Blurt: **Settings → Audio → Rescan**, choose **BlackHole 2ch**, then **Test virtual microphone**.
3. In Discord/Zoom choose **BlackHole 2ch** as the microphone.
4. Turn on **Mix in my microphone**; macOS asks for microphone permission the first time.

### Troubleshooting

| Symptom | Fix |
|---|---|
| Pill says *Set up mic* / *Mic offline* | No virtual cable selected, or it disappeared. Rescan; reinstall the driver after OS updates. |
| Pill says *Mic not reached* | The last test showed this device doesn't feed a microphone (e.g. NVIDIA Broadcast speakers). Use a virtual cable. |
| Test says *No signal* | Blurt's output must be the cable's **playback** side (e.g. *CABLE Input*); the other app must use the **recording** side (*CABLE Output*). |
| Test says *faint* | Raise the cable's level in *Sound settings → Recording/Input* (Windows) or *Audio MIDI Setup* (macOS). |
| Discord chops sounds | Disable Noise Suppression, Echo Cancellation and automatic input sensitivity in Discord. |
| Overlay doesn't appear over my game | Some games in true exclusive fullscreen don't let other windows on top. Keys 1–9 still play while it's open. Switch the game to *windowed fullscreen / borderless* (Valorant: Settings → Video → General → Display Mode → Windowed Fullscreen). Blurt won't inject into games: anti-cheat like Vanguard bans that. |
| Shortcut doesn't work | Another app owns it; Blurt shows a warning. Pick a different combination. |

Logs: `%LOCALAPPDATA%\app.blurt.desktop\logs` (Windows), `~/Library/Logs/app.blurt.desktop` (macOS).
Data: `%APPDATA%\app.blurt.desktop` / `~/Library/Application Support/app.blurt.desktop` (database, `library/`, `tools/`).

## Development

Prerequisites: Node 22+, pnpm, Rust stable, plus the platform's [Tauri prerequisites](https://tauri.app/start/prerequisites/) (MSVC Build Tools + WebView2 on Windows; Xcode Command Line Tools on macOS).

```sh
pnpm install
pnpm tauri dev          # run the app with hot reload
pnpm typecheck          # TypeScript
pnpm test               # frontend unit tests (Vitest)
pnpm test:rust          # Rust unit tests (database, mixer, decoder, routing, downloader, backup…)
pnpm test:hardware      # real-device tests: needs a virtual cable installed (see below)
```

### Production builds

```sh
pnpm release:win                                   # Windows: tauri build + scripts/package.mjs → dist/Blurt-Setup.exe, .msi, SHA256SUMS.txt
pnpm tauri build                                   # Windows: NSIS .exe + .msi in src-tauri/target/release/bundle/
pnpm tauri build --target aarch64-apple-darwin     # macOS Apple Silicon .app + .dmg
pnpm tauri build --target x86_64-apple-darwin      # macOS Intel
```

`build/` is the compiled frontend (Vite). `dist/` holds the release artifacts. `scripts/package.mjs` checks each installer's file format, copies it to a stable name, writes checksums, and updates `site/release.js`.

CI (`.github/workflows/build.yml`) builds Windows x64 and macOS arm64/x64 on every push. Pushing a `v*` tag runs `release.yml`, which opens a **draft** GitHub release with the installers and checksums (see [`docs/RELEASING.md`](docs/RELEASING.md)). Neither workflow has been run yet. There is no auto-updater until builds are signed (see [`docs/UPDATES.md`](docs/UPDATES.md)).

### Code signing & notarization

* **Windows:** set `bundle.windows.certificateThumbprint` (or a `signCommand`) in `tauri.conf.json` with an Authenticode certificate. Unsigned builds trigger SmartScreen.
* **macOS:** add the repository secrets `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` (Developer ID Application), `APPLE_ID`, `APPLE_PASSWORD` (app-specific password) and `APPLE_TEAM_ID`. `tauri build` signs, notarizes and staples. `src-tauri/Info.plist` declares `NSMicrophoneUsageDescription`, required for passthrough and the mic test.

## Testing

| Layer | What | How |
|---|---|---|
| Rust unit (44) | device classification by direction and OS metadata, loopback pairing, combined output gain, migrations, repositories, settings validation, decoder/resampler/waveform, cache, mixer (mixing, trims, fades, pause, limiter, downmix), passthrough (priming, resampling, underrun, drift), cable detection, Goertzel/verify analysis, shortcut canonicalisation, URL validation, yt-dlp error mapping, metadata parsing, import/duplicate/delete, backup round trip | `pnpm test:rust` |
| Hardware (4, `#[ignore]`) | real devices classified by direction; tone through `routing::verify`; a sound through the **real engine** into a virtual cable, captured on its microphone side; `probe_pair` (`BLURT_OUT`/`BLURT_IN`) to test any device pair | `pnpm test:hardware` |
| Frontend unit (8) | view filtering/sorting/search, reorder rules, formatting, shortcut capture, capture resolution & routing state | `pnpm test` |
| Manual / E2E | Verified on Windows 11 by driving the real WebView over CDP: device selection & wrong-direction rejection, NVIDIA Broadcast test failure, import, play into mic, Esc stop-all, mic test, editor, YouTube download → review → add → play, global hotkey from another app, close-to-tray, single instance, autostart registry; installer install → upgrade → uninstall lifecycle | see `docs/IMPLEMENTATION_STATUS.md` |

Can't be automated in CI: anything needing an audio device or virtual cable, OS hotkeys, tray menus, the macOS microphone permission prompt, and real calls in Discord/Zoom. These are listed per feature in `docs/IMPLEMENTATION_STATUS.md`.

## Documentation

* [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md): design, ADRs (virtual mic strategy, audio engine, data, downloader, desktop, device classification, distribution)
* [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md): what's done, what's verified where, known limitations
* [`docs/ROADMAP.md`](docs/ROADMAP.md): what's next
* [`docs/RELEASING.md`](docs/RELEASING.md): building installers, first GitHub setup, cutting a release
* [`docs/UPDATES.md`](docs/UPDATES.md): why there's no auto-updater yet, and what it needs
* [`CHANGELOG.md`](CHANGELOG.md)
* [`docs/BRAND.md`](docs/BRAND.md): name, logo, colour, type

## Legal

MIT licensed. Blurt does not bundle virtual audio drivers or yt-dlp; they're installed separately under their own licences. Only download content you have the right to use. Blurt doesn't sign in to sites, use cookies, or bypass DRM.
