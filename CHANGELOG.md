# Changelog

## [0.4.1] - 2026-11-07

- **Repository and website:** the source is on GitHub, and the landing page links to it. Each release publishes its installer and checksums there.
- **Windows only for now:** macOS builds are paused until they have been tested on a Mac. The app itself is unchanged from 0.4.0.

## [0.4.0] - 2026-11-07

New **Settings → Stream & game** tab. Everything here is off until you turn it on.

- **Auto push-to-talk.** Blurt holds your push-to-talk key (a key, or middle / side mouse button) while a sound plays, so push-to-talk chats in Discord and games actually send it. Keys are sent as scan codes. Some anti-cheat games ignore simulated keys.
- **Instant clips.** Keeps the last 5–120 s of what you hear (game, friends, music) in memory. Ctrl+Alt+C or *Clip now* saves it to your library as “Clip #N”. Nothing touches the disk until you clip.
- **Stream Deck, Touch Portal and Streamer.bot control.** A private local link (127.0.0.1:7270, secret token) with /play?name=…, /random, /replay, /stop, /clip, /overlay and /sounds. Use Stream Deck’s *Website* action with “GET request in background”.
- **Text to speech.** The *Speak* button says what you type into the mic with the Windows voices. Each line is saved as a sound you can replay or bind to a key.

## [0.3.0] - 2026-11-07

- **Replay last sound** with a global shortcut (default Ctrl+Alt+,).
- **In-game overlay** (default Ctrl+Alt+B): your pinned sounds first (in your order), then favourites and most played, on keys 1–9 and 0. It never takes focus, so the game stays fullscreen in front and the taskbar doesn't appear. It closes by itself after 4 s without a key, or with the shortcut again. Esc also closes it outside games that keep Esc (such as Valorant). The playing sound is highlighted. Blurt never injects into games.
- **Play a random sound** shortcut (default Ctrl+Alt+R), never the same sound twice in a row.
- **Lower my voice while a sound plays**: with mic passthrough on, your mic drops to a quarter (−12 dB) during sounds and fades back smoothly.
- **One-click setup.** Onboarding is now one button, *Set up my microphone*: it finds and tests a working virtual microphone, fixes its level, and mixes your real mic in. If the PC has none, Blurt downloads VB-CABLE from vb-audio.com, checks VB-Audio's signature, and opens VB-Audio's own installer. It then sets everything up, or finishes on the next launch if Windows needs a restart. Manual device choice is under *Choose devices myself*.

## [0.2.1] - 2026-11-06

- **Fixed:** AIFF files were listed as supported but failed to import (the decoder feature was missing). Verified MP3, WAV, OGG, FLAC, M4A (AAC and ALAC), AIFF and WebM/Vorbis with real files; WebM/Opus and WMA are refused with a clear message.
- **Set up automatically.** One button tests every virtual audio device on the PC, chooses one that really reaches a microphone, fixes its level, and mixes your own mic in.
- **Fix level.** If a virtual microphone works but is too quiet, Blurt raises its Windows input level by the measured loss, keeping 6 dB of headroom. Steam Streaming Microphone loses about 26–32 dB on its own.
- **Real speakers and microphones are called out.** Choosing your speakers (for example Speakers (Realtek)) as the device Blurt plays into now shows "Mic not reached" with an explanation, and plays warn about it. Previously everything showed green.
- **Much faster device handling:** devices are listed through Windows' endpoint API (2.2 s → about 10 ms here), so the virtual mic is ready about 1 s after launch instead of about 5.7 s. Plays that arrive while it is still opening wait for it (up to 3 s) instead of failing.
- **Fixed:** device metadata now uses the same names apps see. VB-CABLE was only recognised by its name; a renamed virtual device lost its classification.
- **Fixed:** the audio status could show "unavailable" right after a routing change even though the device was open.

## [0.2.0] - 2026-11-05

- **Audio routing rebuilt around real device metadata.** Blurt now reads each endpoint's actual direction and owning driver from Windows, instead of guessing from names. Playback and recording devices are chosen separately.
- **Better virtual microphone test.** It reports four things separately: playback device, virtual microphone, signal received, and signal quality (level and noise floor). Results are remembered per device pair.
- **NVIDIA Broadcast guidance.** Its speaker device is no longer mislabelled. The test shows it doesn't feed its microphone, and Blurt explains the setup that does work: a virtual cable, with Microphone (NVIDIA Broadcast) mixed in via passthrough.
- **New combined output volume** alongside the soundboard and microphone levels.
- **Device changes are detected automatically.** Plugging in, unplugging and reinstalling devices, or changing the default device, refreshes the list and reconnects. Your choices are kept.
- **Installer.** Installs into Program Files for all users, with Start Menu and optional desktop shortcuts. Your library is kept on upgrade and uninstall.

## [0.1.0] - 2026-11-05

- First version: soundboard, virtual microphone routing, imports, downloads, categories and collections, global shortcuts, tray.
