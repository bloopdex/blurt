import { create } from "zustand";
import { toast } from "sonner";

import { api, errorMessage, type Group, type Sound, type SoundPatch } from "@/lib/api";

interface LibraryStore {
  sounds: Sound[];
  groups: Group[];
  loaded: boolean;
  query: string;
  /** Files waiting in the import dialog (from the picker or a drag-and-drop). */
  pendingImport: string[] | null;
  editingId: string | null;
  load: () => Promise<void>;
  setQuery: (q: string) => void;
  setPendingImport: (paths: string[] | null) => void;
  setEditingId: (id: string | null) => void;
  upsert: (s: Sound) => void;
  importFiles: (paths: string[], categoryId: string | null) => Promise<void>;
  update: (id: string, patch: SoundPatch) => Promise<Sound | null>;
  remove: (ids: string[]) => Promise<void>;
  duplicate: (id: string) => Promise<void>;
  reorder: (ids: string[]) => Promise<void>;
  move: (ids: string[], categoryId: string | null) => Promise<void>;
  setMembership: (collectionId: string, ids: string[], member: boolean) => Promise<void>;
  markPlayed: (id: string, playCount: number, lastPlayedAt: number | null) => void;
  createGroup: (kind: Group["kind"], name: string, icon: string, color: string) => Promise<Group | null>;
  updateGroup: (id: string, name: string, icon: string, color: string) => Promise<boolean>;
  deleteGroup: (id: string) => Promise<void>;
  reorderGroups: (ids: string[]) => Promise<void>;
}

function fail(title: string, e: unknown) {
  toast.error(title, { description: errorMessage(e) });
}

export const useLibrary = create<LibraryStore>((set, get) => ({
  sounds: [],
  groups: [],
  loaded: false,
  query: "",
  pendingImport: null,
  editingId: null,

  load: async () => {
    try {
      const { sounds, groups } = await api.getLibrary();
      set({ sounds, groups, loaded: true });
    } catch (e) {
      set({ loaded: true });
      fail("Couldn't load your library", e);
    }
  },

  setQuery: (query) => set({ query }),
  setPendingImport: (pendingImport) => set({ pendingImport }),
  setEditingId: (editingId) => set({ editingId }),

  upsert: (s) =>
    set((st) => ({
      sounds: st.sounds.some((x) => x.id === s.id) ? st.sounds.map((x) => (x.id === s.id ? s : x)) : [...st.sounds, s],
    })),

  importFiles: async (paths, categoryId) => {
    const t = toast.loading(`Importing ${paths.length} file${paths.length === 1 ? "" : "s"}…`);
    try {
      const report = await api.importFiles(paths, categoryId);
      set((st) => ({ sounds: [...st.sounds, ...report.imported] }));
      toast.dismiss(t);
      if (report.imported.length) {
        toast.success(
          report.imported.length === 1
            ? `Added “${report.imported[0].name}”`
            : `Added ${report.imported.length} sounds`,
        );
      }
      for (const f of report.failed) {
        const name = f.path.split(/[\\/]/).pop();
        toast.error(`Couldn't import ${name}`, { description: f.message, duration: 8000 });
      }
    } catch (e) {
      toast.dismiss(t);
      fail("Import failed", e);
    }
  },

  update: async (id, patch) => {
    const prev = get().sounds.find((s) => s.id === id);
    if (prev) get().upsert({ ...prev, ...patch });
    try {
      const s = await api.updateSound(id, patch);
      get().upsert(s);
      return s;
    } catch (e) {
      if (prev) get().upsert(prev);
      fail("Couldn't save changes", e);
      return null;
    }
  },

  remove: async (ids) => {
    try {
      await api.deleteSounds(ids);
      set((st) => ({ sounds: st.sounds.filter((s) => !ids.includes(s.id)) }));
      toast.success(ids.length === 1 ? "Sound deleted" : `${ids.length} sounds deleted`, {
        description: "The audio file was moved to Blurt's trash (Settings → Storage).",
      });
    } catch (e) {
      fail("Couldn't delete", e);
      await get().load();
    }
  },

  duplicate: async (id) => {
    try {
      get().upsert(await api.duplicateSound(id));
    } catch (e) {
      fail("Couldn't duplicate", e);
    }
  },

  reorder: async (ids) => {
    const byId = new Map(get().sounds.map((s) => [s.id, s]));
    const reordered = ids.flatMap((id, position) => {
      const s = byId.get(id);
      return s ? [{ ...s, position }] : [];
    });
    set({ sounds: reordered });
    try {
      await api.reorderSounds(ids);
    } catch (e) {
      fail("Couldn't save the new order", e);
      await get().load();
    }
  },

  move: async (ids, categoryId) => {
    set((st) => ({ sounds: st.sounds.map((s) => (ids.includes(s.id) ? { ...s, categoryId } : s)) }));
    try {
      await api.moveSounds(ids, categoryId);
    } catch (e) {
      fail("Couldn't move", e);
      await get().load();
    }
  },

  setMembership: async (collectionId, ids, member) => {
    set((st) => ({
      sounds: st.sounds.map((s) => {
        if (!ids.includes(s.id)) return s;
        const rest = s.collectionIds.filter((c) => c !== collectionId);
        return { ...s, collectionIds: member ? [...rest, collectionId] : rest };
      }),
    }));
    try {
      await api.setCollectionMembership(collectionId, ids, member);
    } catch (e) {
      fail("Couldn't update the collection", e);
      await get().load();
    }
  },

  markPlayed: (id, playCount, lastPlayedAt) =>
    set((st) => ({ sounds: st.sounds.map((s) => (s.id === id ? { ...s, playCount, lastPlayedAt } : s)) })),

  createGroup: async (kind, name, icon, color) => {
    try {
      const g = await api.createGroup(kind, name, icon, color);
      set((st) => ({ groups: [...st.groups, g] }));
      return g;
    } catch (e) {
      fail(`Couldn't create the ${kind}`, e);
      return null;
    }
  },

  updateGroup: async (id, name, icon, color) => {
    try {
      await api.updateGroup(id, name, icon, color);
      set((st) => ({ groups: st.groups.map((g) => (g.id === id ? { ...g, name: name.trim(), icon, color } : g)) }));
      return true;
    } catch (e) {
      fail("Couldn't save", e);
      return false;
    }
  },

  deleteGroup: async (id) => {
    try {
      await api.deleteGroup(id);
      set((st) => ({
        groups: st.groups.filter((g) => g.id !== id),
        sounds: st.sounds.map((s) => ({
          ...s,
          categoryId: s.categoryId === id ? null : s.categoryId,
          collectionIds: s.collectionIds.filter((c) => c !== id),
        })),
      }));
    } catch (e) {
      fail("Couldn't delete", e);
    }
  },

  reorderGroups: async (ids) => {
    set((st) => ({
      groups: st.groups.map((g) => (ids.includes(g.id) ? { ...g, position: ids.indexOf(g.id) } : g)),
    }));
    try {
      await api.reorderGroups(ids);
    } catch (e) {
      fail("Couldn't save the new order", e);
      await get().load();
    }
  },
}));
