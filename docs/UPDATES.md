# Automatic updates — readiness plan

Blurt doesn't ship an updater yet. "Check for updates" in About is manual on purpose: **an update mechanism without signature verification is worse than none.** When we turn it on, it will be the official `tauri-plugin-updater`, which refuses any update whose signature doesn't verify.

## 1. Signing keys

Updates are signed with an **Ed25519 (minisign) key pair**. This is separate from Windows Authenticode and Apple notarization, which you also want.

```sh
pnpm tauri signer generate -w ~/.tauri/blurt-updater.key
```

* **Private key + password** → repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Never commit them. Keep an offline backup: losing the key means existing installs can never be updated again (users would have to reinstall manually).
* **Public key** → committed in `tauri.conf.json` (`plugins.updater.pubkey`), and therefore compiled into every build.

## 2. Configuration

```jsonc
// src-tauri/tauri.conf.json
"bundle": { "createUpdaterArtifacts": true },
"plugins": {
  "updater": {
    "pubkey": "<contents of blurt-updater.key.pub>",
    "endpoints": ["https://github.com/OWNER/REPO/releases/latest/download/latest.json"],
    "windows": { "installMode": "passive" }
  }
}
```

Add `tauri-plugin-updater` (Rust) and `@tauri-apps/plugin-updater` (JS), plus the `updater:default` capability.

## 3. Update manifest (`latest.json`)

`tauri build` with `createUpdaterArtifacts` produces a `.sig` next to each installer. The release job assembles:

```json
{
  "version": "0.3.0",
  "notes": "…from CHANGELOG.md…",
  "pub_date": "2026-12-01T12:00:00Z",
  "platforms": {
    "windows-x86_64": { "signature": "<contents of Blurt_0.3.0_x64-setup.exe.sig>", "url": "https://github.com/OWNER/REPO/releases/download/v0.3.0/Blurt-Setup.exe" },
    "darwin-aarch64": { "signature": "…", "url": "…/Blurt-macOS-arm64.app.tar.gz" },
    "darwin-x86_64":  { "signature": "…", "url": "…/Blurt-macOS-x64.app.tar.gz" }
  }
}
```

## 4. Hosting

GitHub Releases is enough. `releases/latest/download/latest.json` always points at the newest *published* release; drafts are invisible to clients, so the draft-then-publish flow in `release.yml` doubles as a release gate.

## 5. Verification

The client downloads `latest.json` over HTTPS, compares versions, downloads the installer, and **verifies its Ed25519 signature against the embedded public key before running anything**. A bad or missing signature aborts the update. HTTPS protects the transport; the signature protects against a compromised release host.

## 6. Installation flow

1. On launch (and from About → Check for updates), call `check()`; if newer, show the notes and an **Update** button. Never install silently without consent.
2. `downloadAndInstall()` with a progress bar.
3. Windows: the NSIS installer runs in passive mode with `/UPDATE`, which upgrades in place and **never deletes app data** (the template skips the delete-data step in update mode). macOS: the `.app` is replaced.
4. Restart into the new version. The library lives in the app-data folder, not the install folder, so it survives (verified for manual upgrades — see IMPLEMENTATION_STATUS.md).

## Checklist to enable

- [ ] Generate the key pair; store the secrets and an offline backup
- [ ] Add the plugin, config and capability; wire the About button
- [ ] Extend `release.yml` to upload `.sig` files and `latest.json`
- [ ] Test an upgrade from the previous published version on Windows and macOS
- [ ] Test that a tampered installer (bad signature) is rejected
