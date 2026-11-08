// Talks to the Blurt desktop app, which listens on this PC only. Blurt accepts these requests
// because their Origin is this extension; web pages can't send them.
const BLURT = "http://127.0.0.1:7270/extension/";

export const SITES = [
  "youtube.com", "youtu.be", "tiktok.com", "instagram.com", "x.com", "twitter.com",
  "soundcloud.com", "reddit.com", "twitch.tv", "vimeo.com",
];
export const PATTERNS = SITES.flatMap((h) => [`*://${h}/*`, `*://*.${h}/*`]);

const NOT_RUNNING = "Blurt isn't running. Open the Blurt app, then try again.";

/** Is Blurt running and accepting the extension? Resolves to { ok, version } or { ok: false, error }. */
export async function ping() {
  try {
    // POST: browsers only attach the extension Origin (which Blurt requires) to non-GET requests.
    const r = await fetch(BLURT + "ping", { method: "POST" });
    const body = await r.json().catch(() => ({}));
    return r.ok ? { ok: true, version: body.version } : { ok: false, error: body.error || `Blurt answered ${r.status}.` };
  } catch {
    return { ok: false, error: NOT_RUNNING };
  }
}

/** Ask Blurt to download `url`. Resolves to { ok, title } or { ok: false, error }. */
export async function send(url) {
  if (!/^https?:\/\//.test(url || "")) return { ok: false, error: "Open a video page first." };
  try {
    const r = await fetch(BLURT + "send?url=" + encodeURIComponent(url), { method: "POST" });
    const body = await r.json().catch(() => ({}));
    return r.ok ? { ok: true, title: body.title } : { ok: false, error: body.error || `Blurt answered ${r.status}.` };
  } catch {
    return { ok: false, error: NOT_RUNNING };
  }
}

export function isSupported(url) {
  try {
    const host = new URL(url).hostname.toLowerCase();
    return SITES.some((h) => host === h || host.endsWith("." + h));
  } catch {
    return false;
  }
}
