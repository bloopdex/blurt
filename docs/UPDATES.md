# Automatic updates

Blurt uses the official `tauri-plugin-updater`. At startup (if *Check for updates automatically* is on), and when you press *Check for updates* in Settings → About, it reads

    https://github.com/bloopdex/blurt/releases/latest/download/latest.json

If that lists a newer version, Blurt offers **Update now**. The plugin downloads the installer and **verifies its signature against the public key compiled into Blurt** (`plugins.updater.pubkey` in `tauri.conf.json`). Only then does it run the installer (NSIS, passive mode: a progress window, and a UAC prompt because Blurt installs per machine). The installer closes Blurt and starts the new version. Library and settings are untouched.

## Signing key

* An **Ed25519 (minisign) key pair**, separate from Windows Authenticode. Created with `pnpm tauri signer generate`.
* **Private key + password:** repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, used only by `.github/workflows/release.yml`. A local copy lives in `~/.tauri/blurt-updater.key` and `~/.tauri/blurt-updater.password` on the maintainer's PC.
* **Back up both files somewhere safe and offline.** If the key is lost, installed copies can never verify another update, and everyone has to reinstall by hand. If the key leaks, an attacker could sign "updates": rotate it by publishing one last release signed with the old key that ships a new public key.

## How a release produces an update

1. `tauri build` with `bundle.createUpdaterArtifacts: true` and the private key writes `Blurt_X_x64-setup.exe.sig` next to the installer.
2. `scripts/package.mjs` copies the installer to `dist/Blurt-Setup.exe` and writes `dist/latest.json` (version, notes from `CHANGELOG.md`, signature, and the URL of *this version's* `Blurt-Setup.exe`). The release job fails if `latest.json` is missing.
3. The release publishes both. Because it's marked *latest*, `releases/latest/download/latest.json` now points at it.

CI builds (`build.yml`) skip update signing with `--config '{"bundle":{"createUpdaterArtifacts":false}}'`, so pull requests don't need the key.

To build a signed release locally:

```sh
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/blurt-updater.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/blurt-updater.password)" \
pnpm release:win
```

## Privacy

The update check is a plain request for `latest.json` on github.com. No identifier, library or usage data is sent. It's disclosed in the [privacy policy](https://blurt.bloopdex.com/privacy.html) and can be turned off.
