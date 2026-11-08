import { PATTERNS, send } from "./blurt.js";

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({ id: "page", title: "Send this video to Blurt", contexts: ["page", "video"], documentUrlPatterns: PATTERNS });
  chrome.contextMenus.create({ id: "link", title: "Send link to Blurt", contexts: ["link"], targetUrlPatterns: PATTERNS });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  void sendAndShow(info.menuItemId === "link" ? info.linkUrl : info.pageUrl, tab?.id);
});

chrome.commands.onCommand.addListener(async (command, tab) => {
  if (command === "send-current-tab" && tab) await sendAndShow(tab.url, tab.id);
});

/** Badge feedback for sends that don't go through the popup: ✓ sent, ! failed (hover shows why). */
async function sendAndShow(url, tabId) {
  const r = await send(url);
  await chrome.action.setBadgeBackgroundColor({ tabId, color: r.ok ? "#4CAF50" : "#E5484D" });
  await chrome.action.setBadgeText({ tabId, text: r.ok ? "✓" : "!" });
  await chrome.action.setTitle({ tabId, title: r.ok ? `Sent to Blurt: ${r.title}` : r.error });
  setTimeout(() => {
    chrome.action.setBadgeText({ tabId, text: "" });
    chrome.action.setTitle({ tabId, title: "Send to Blurt" });
  }, 5000);
}
