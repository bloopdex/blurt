import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { toast } from "sonner";

import { errorMessage } from "./api";

export const PRIVACY_URL = "https://blurt.bloopdex.com/privacy.html";
export const TERMS_URL = "https://blurt.bloopdex.com/terms.html";

/** Ask GitHub for a newer, signed release. Quiet when automatic; reports every outcome when `manual`. */
export async function checkForUpdates(manual: boolean) {
  let update: Update | null;
  try {
    update = await check();
  } catch (e) {
    if (manual) toast.error("Couldn't check for updates", { description: errorMessage(e) });
    return;
  }
  if (!update) {
    if (manual) toast.success("Blurt is up to date");
    return;
  }
  const found = update;
  toast.info(`Blurt ${found.version} is available`, {
    id: "update",
    description: "Your sounds and settings are kept. Blurt restarts after updating.",
    duration: Infinity,
    action: { label: "Update now", onClick: () => void install(found) },
  });
}

/** The plugin verifies the download's signature before installing anything. */
async function install(update: Update) {
  const id = "update";
  let total = 0;
  let received = 0;
  toast.loading("Downloading update…", { id, duration: Infinity });
  try {
    await update.downloadAndInstall((ev) => {
      if (ev.event === "Started") total = ev.data.contentLength ?? 0;
      else if (ev.event === "Progress") {
        received += ev.data.chunkLength;
        if (total) toast.loading(`Downloading update… ${Math.round((received / total) * 100)}%`, { id, duration: Infinity });
      } else toast.loading("Installing update…", { id, duration: Infinity });
    });
    // On Windows the installer closes Blurt and starts the new version itself; this covers the rest.
    await relaunch();
  } catch (e) {
    toast.error("The update didn't install", { id, description: errorMessage(e), duration: 10000 });
  }
}
