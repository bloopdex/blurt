import { create } from "zustand";
import { toast } from "sonner";

import { api, errorMessage, type DownloadRecord, type ToolStatus } from "@/lib/api";

interface DownloadsStore {
  items: DownloadRecord[];
  progress: Record<string, number>;
  tool: ToolStatus | null;
  load: () => Promise<void>;
  loadTool: () => Promise<void>;
  setProgress: (id: string, percent: number) => void;
  finished: (id: string) => Promise<void>;
  add: (d: DownloadRecord) => void;
  remove: (id: string) => Promise<void>;
}

export const useDownloads = create<DownloadsStore>((set, get) => ({
  items: [],
  progress: {},
  tool: null,

  load: async () => {
    try {
      set({ items: await api.listDownloads() });
    } catch (e) {
      toast.error("Couldn't load download history", { description: errorMessage(e) });
    }
  },

  loadTool: async () => {
    try {
      set({ tool: await api.ytdlpStatus() });
    } catch (e) {
      toast.error("Couldn't check the downloader", { description: errorMessage(e) });
    }
  },

  setProgress: (id, percent) => set((st) => ({ progress: { ...st.progress, [id]: percent } })),

  finished: async (id) => {
    await get().load();
    const d = get().items.find((x) => x.id === id);
    if (!d) return;
    const title = d.title ?? "Download";
    if (d.status === "ready") toast.success(`“${title}” downloaded`, { description: "Review it and add it to your board." });
    else if (d.status === "failed") toast.error(`Couldn't download “${title}”`, { description: d.error ?? undefined, duration: 10000 });
  },

  add: (d) => set((st) => ({ items: [d, ...st.items.filter((x) => x.id !== d.id)] })),

  remove: async (id) => {
    try {
      await api.deleteDownload(id);
      set((st) => ({ items: st.items.filter((x) => x.id !== id) }));
    } catch (e) {
      toast.error("Couldn't remove the download", { description: errorMessage(e) });
    }
  },
}));
