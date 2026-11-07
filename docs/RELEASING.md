# Releasing Blurt

## Build a Windows installer locally

```sh
pnpm install
pnpm release:win        # tauri build + scripts/package.mjs
```

The output goes to `dist/`:

| File | What it is |
|---|---|
| `Blurt-Setup.exe` | NSIS installer: per-machine into Program Files, Start Menu + optional desktop shortcut, uninstaller. **This is the one to hand people.** |
| `Blurt-Setup.msi` | WiX MSI, for managed or enterprise deployment |
| `SHA256SUMS.txt` | Checksums (`sha256sum -c SHA256SUMS.txt`) |

`package.mjs` refuses a file whose format signature is wrong, so a successful run means each file really is an installer. It also writes `site/release.js` (git-ignored), which the landing page reads. For a local preview, set `downloadBase` in `site/config.js` to `"../dist/"`.

Silent install for testing or IT: `Blurt-Setup.exe /S` (needs an elevated prompt). Silent uninstall: `"C:\Program Files\Blurt\uninstall.exe" /S` (keeps user data).

## Where things live

* Repository: <https://github.com/bloopdex/blurt> (`git@github.com:bloopdex/blurt.git`)
* Releases: <https://github.com/bloopdex/blurt/releases>. The stable download link is <https://github.com/bloopdex/blurt/releases/latest/download/Blurt-Setup.exe>.
* Landing page: <https://bloopdex.github.io/blurt/>. GitHub Pages is set to *Source: GitHub Actions*, and the `github-pages` environment allows `v*` tags to deploy.
* CI: `.github/workflows/build.yml` runs on every push and pull request. Windows must pass. macOS builds are informational until verified.

## Cutting a release

1. Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`, and add a `## [x.y.z]` section to `CHANGELOG.md`.
2. Commit, then tag and push:
   ```sh
   git tag v0.2.0 && git push origin main v0.2.0
   ```
3. `.github/workflows/release.yml` checks that the tag matches the app version, runs the tests, builds Windows x64, verifies each installer's format, and **publishes** the release with `Blurt-Setup.exe`, `Blurt-Setup.msi` and `SHA256SUMS.txt` (notes taken from `CHANGELOG.md`). It then deploys `site/` to GitHub Pages, together with that build's `release.js`.
4. Download the installer from the release, install it over the previous version, and check that your library is still there.

macOS is left out of releases until someone verifies a build on a Mac. Then add it back to the release matrix and set `macosAvailable: true` in `site/config.js`.

## Release checklist

- [ ] `pnpm typecheck && pnpm test && pnpm test:rust` green
- [ ] `pnpm test:hardware` on a machine with a virtual cable
- [ ] Installer: fresh install, upgrade over the previous version, uninstall (data kept), launch from Start Menu
- [ ] Signing: Authenticode (Windows) and notarization (macOS), when certificates exist
