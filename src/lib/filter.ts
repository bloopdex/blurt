import type { Settings, Sound } from "./api";
import type { View } from "@/store/app";

/** Which sounds a view shows, filtered by the search query and sorted. Pinned sounds lead. */
export function visibleSounds(sounds: Sound[], view: View, query: string, sort: Settings["sort"]): Sound[] {
  let list = sounds.filter((s) => {
    switch (view.kind) {
      case "favorites":
        return s.isFavorite;
      case "recent":
        return s.lastPlayedAt !== null;
      case "downloaded":
        return s.sourceUrl !== null;
      case "category":
        return s.categoryId === view.id;
      case "collection":
        return s.collectionIds.includes(view.id);
      default:
        return true;
    }
  });

  const q = query.trim().toLowerCase();
  if (q) {
    const terms = q.split(/\s+/);
    list = list.filter((s) => {
      const hay = `${s.name} ${s.description} ${s.originalName}`.toLowerCase();
      return terms.every((t) => hay.includes(t));
    });
  }

  if (view.kind === "recent") {
    return list.sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0)).slice(0, 60);
  }

  const cmp: Record<Settings["sort"], (a: Sound, b: Sound) => number> = {
    manual: (a, b) => a.position - b.position,
    name: (a, b) => a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" }),
    recent: (a, b) => b.createdAt - a.createdAt,
    plays: (a, b) => b.playCount - a.playCount || a.position - b.position,
    duration: (a, b) => effectiveDuration(a) - effectiveDuration(b),
  };
  return list.sort((a, b) => Number(b.isPinned) - Number(a.isPinned) || cmp[sort](a, b));
}

export function effectiveDuration(s: Sound): number {
  return (s.trimEndMs ?? s.durationMs) - s.trimStartMs;
}

/** Manual drag-to-reorder only makes sense when the grid shows the stored order. */
export function canReorder(view: View, query: string, sort: Settings["sort"]): boolean {
  return sort === "manual" && !query.trim() && view.kind !== "recent";
}
