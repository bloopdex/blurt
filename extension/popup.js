import { isSupported, ping, send } from "./blurt.js";

const $ = (id) => document.getElementById(id);
const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
const url = tab?.url ?? "";
$("page").textContent = tab?.title || url || "No page";

const show = (el, text, cls) => {
  el.textContent = text;
  el.className = el.className.split(" ")[0] + (cls ? " " + cls : "");
};

const alive = await ping();
show($("status"), alive.ok ? "Blurt is running" : "Blurt not found", alive.ok ? "ok" : "bad");
if (!alive.ok) show($("result"), alive.error, "bad");
else if (!isSupported(url)) show($("result"), "Open a YouTube, TikTok, Instagram, X, SoundCloud, Reddit, Twitch or Vimeo video.", "");
$("send").disabled = !alive.ok || !/^https?:/.test(url);

$("send").addEventListener("click", async () => {
  $("send").disabled = true;
  $("send").textContent = "Sending…";
  const r = await send(url);
  show($("result"), r.ok ? `Downloading “${r.title}” in Blurt. Trim it and add it from Downloads.` : r.error, r.ok ? "ok" : "bad");
  $("send").textContent = r.ok ? "Sent ✓" : "Send to Blurt";
  $("send").disabled = r.ok;
});
