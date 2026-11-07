import { toast } from "sonner";

import { api, errorMessage } from "./api";
import { useApp } from "@/store/app";

/** Play on the board. A missing virtual mic is reported every time rather than hidden. */
export async function playSound(id: string) {
  try {
    const outcome = await api.playSound(id);
    if (outcome.warning) {
      toast.warning("Only you can hear this", {
        id: "routing-warning",
        description: outcome.warning,
        action: {
          label: "Audio setup",
          onClick: () => useApp.getState().setView({ kind: "settings", tab: "audio" }),
        },
      });
    }
  } catch (e) {
    toast.error("Couldn't play that sound", { id: "play-error", description: errorMessage(e) });
  }
}

export async function stopAll() {
  try {
    await api.stopAll();
  } catch (e) {
    toast.error("Couldn't stop playback", { description: errorMessage(e) });
  }
}

/** Wrap a preview call (monitor only) with error reporting. */
export async function preview(run: () => Promise<number | null>) {
  try {
    return await run();
  } catch (e) {
    toast.error("Couldn't preview", { description: errorMessage(e) });
    return null;
  }
}
