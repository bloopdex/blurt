import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

import { confirm } from "@/components/ConfirmDialog";
import { useApp } from "@/store/app";
import { useLibrary } from "@/store/library";
import { api, errorMessage } from "./api";

export const audioExtensions = ["mp3", "wav", "ogg", "oga", "flac", "m4a", "mp4", "aac", "aiff", "aif", "webm", "mka", "caf"];

export async function deleteSounds(ids: string[]) {
  const { sounds, remove } = useLibrary.getState();
  if (useApp.getState().settings?.confirmDelete ?? true) {
    const name = ids.length === 1 ? `“${sounds.find((s) => s.id === ids[0])?.name ?? "this sound"}”` : `${ids.length} sounds`;
    const ok = await confirm({
      title: `Delete ${name}?`,
      description: "It's removed from your board and its audio file goes to Blurt's trash until you empty it.",
      confirmLabel: "Delete",
      destructive: true,
    });
    if (!ok) return;
  }
  await remove(ids);
}

/** Native file picker → import dialog. */
export async function pickFilesToImport() {
  try {
    const picked = await open({
      multiple: true,
      title: "Add sounds to Blurt",
      filters: [{ name: "Audio", extensions: audioExtensions }],
    });
    if (picked && picked.length) useLibrary.getState().setPendingImport(picked);
  } catch (e) {
    toast.error("Couldn't open the file picker", { description: errorMessage(e) });
  }
}

export async function pickCover(soundId: string) {
  try {
    const picked = await open({
      multiple: false,
      title: "Choose a cover image",
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    if (!picked) return;
    useLibrary.getState().upsert(await api.setSoundCover(soundId, picked));
  } catch (e) {
    toast.error("Couldn't set the cover", { description: errorMessage(e) });
  }
}

export async function clearCover(soundId: string) {
  try {
    useLibrary.getState().upsert(await api.setSoundCover(soundId, null));
  } catch (e) {
    toast.error("Couldn't remove the cover", { description: errorMessage(e) });
  }
}
