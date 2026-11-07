# Roadmap

Phases follow the original brief. ✅ done · 🟡 partly · ⏭ next.

| Phase | State | Notes |
|---|---|---|
| 1 Research & architecture | ✅ | `docs/ARCHITECTURE.md` (ADR-001…007) |
| 2 Foundation (Tauri 2, React, Tailwind, shadcn/ui, SQLite, identity) | ✅ | |
| 3 Soundboard (grid, sidebar, categories, favourites, import, playback, player bar) | ✅ | |
| 4 Audio routing (engine, virtual mic, monitor, passthrough, detection, verification) | 🟡 | v0.2: metadata-based classification + signal test, verified on Windows (Steam cable, NVIDIA Broadcast); macOS and VB-CABLE still to run |
| 5 Downloads (yt-dlp, metadata, review, import) | ✅ | YouTube verified; TikTok/Instagram untested |
| 6 Desktop integration (tray, autostart, global shortcuts, background, lifecycle) | 🟡 | Windows verified; macOS untested |
| 7 Polish (onboarding, settings, errors, a11y, tests) | 🟡 | |
| 8 Distribution (installers, CI, release workflow, download page, update plan) | 🟡 | `dist/Blurt-Setup.exe` built; install lifecycle verified per-user; elevated per-machine install, CI, macOS untested |

## Next, in priority order

1. **Elevated install check:** run `dist/Blurt-Setup.exe` with admin approval to confirm Program Files, the HKLM entry, the all-users Start Menu shortcut and uninstall. The per-user lifecycle already passed.
2. **Publish:** `git init`, push to GitHub, tag `v0.2.0`, review the draft release, set `site/config.js` (see `docs/RELEASING.md`).
3. **macOS bring-up** on a real Mac: build, BlackHole routing test (`pnpm test:hardware`), mic permission prompt, tray/Dock, LaunchAgent, DMG, and Core Audio transport-type metadata in `endpoints.rs`. Only then set `macosAvailable: true` on the download page.
4. **VB-CABLE run** on Windows: install, select, run the in-app test and the hardware tests, then a real Discord call. Include the NVIDIA Broadcast microphone as the passthrough source.
5. **Device notifications:** replace the 5 s poll with `IMMNotificationClient` (Windows) / Core Audio property listeners. Then test unplug, replug, default switch and driver reinstall.
6. **Passthrough in a live call:** latency and drift over a 30-minute session; tune `TARGET_SECS`.
7. **Code signing:** Windows Authenticode certificate; Apple Developer ID and notarization secrets in CI.
8. **Updater:** follow `docs/UPDATES.md` (signed manifest, then wire "Check for updates").
9. Streaming decode for long tracks (lift the 15-minute cap); windowed-sinc resampler.
10. Stable device IDs (WASAPI endpoint IDs) so renamed devices keep working.
11. Localisation scaffolding (i18n) once the copy settles.
12. Optional: a bundled, signed virtual driver to remove the external install step. A macOS AudioServerPlugIn is the more tractable one.
