// Landing page settings.
window.BLURT_SITE = {
  // Where the installers live. Asset names are stable (Blurt-Setup.exe, …), so this is a folder URL.
  // For a local preview of a fresh `pnpm release:win`, use "../dist/".
  downloadBase: "https://github.com/bloopdex/blurt/releases/latest/download/",
  releasesUrl: "https://github.com/bloopdex/blurt/releases",
  repoUrl: "https://github.com/bloopdex/blurt",
  // Only flip this once a macOS build has been tested end to end on a Mac.
  macosAvailable: false,
};
