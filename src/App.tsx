import { useEffect, useState } from "react";
import {
  closestCenter,
  DndContext,
  DragOverlay,
  PointerSensor,
  pointerWithin,
  useSensor,
  useSensors,
  type CollisionDetection,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { arrayMove } from "@dnd-kit/sortable";
import { Music2 } from "lucide-react";
import { toast, Toaster } from "sonner";

import { TooltipProvider } from "@/components/ui/tooltip";
import { ConfirmHost } from "@/components/ConfirmDialog";
import { DropOverlay, ImportDialog } from "@/components/ImportDialog";
import { PlayerBar } from "@/components/layout/PlayerBar";
import { Sidebar } from "@/components/layout/Sidebar";
import { SoundEditor } from "@/components/sounds/SoundEditor";
import type { Group } from "@/lib/api";
import { canReorder } from "@/lib/filter";
import { subscribe } from "@/lib/events";
import { stopAll } from "@/lib/playback";
import { useApp } from "@/store/app";
import { useDownloads } from "@/store/downloads";
import { useLibrary } from "@/store/library";
import { DownloadsView } from "@/views/DownloadsView";
import { LibraryView } from "@/views/LibraryView";
import { Onboarding } from "@/views/Onboarding";
import { autoSetup, FINISH_SETUP_KEY } from "@/components/routing/EasySetup";
import { checkForUpdates } from "@/lib/updates";
import { SettingsView } from "@/views/SettingsView";

// Sidebar targets need the pointer to be inside them; grid reordering uses the nearest card.
const collision: CollisionDetection = (args) => {
  const within = pointerWithin(args).filter((c) => String(c.id).startsWith("group:"));
  return within.length ? within : closestCenter(args);
};

function useTheme() {
  const theme = useApp((s) => s.settings?.theme ?? "dark");
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => document.documentElement.classList.toggle("dark", theme === "dark" || (theme === "system" && media.matches));
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  return theme;
}

export default function App() {
  const { view, loadSettings, settings } = useApp();
  const loadLibrary = useLibrary((s) => s.load);
  const [hoveringFiles, setHoveringFiles] = useState(false);
  const [dragLabel, setDragLabel] = useState<string | null>(null);
  const theme = useTheme();
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));

  useEffect(() => {
    let unsubscribe: (() => void) | undefined;
    let cancelled = false;
    void subscribe(setHoveringFiles).then((u) => (cancelled ? u() : (unsubscribe = u)));
    loadSettings()
      .then(() => useApp.getState().refreshDevices())
      .then(finishSetupAfterRestart)
      .then(() => {
        // A few seconds after launch, so the check never delays startup.
        if (useApp.getState().settings?.autoUpdate) setTimeout(() => void checkForUpdates(false), 5000);
      })
      .catch((e) => toast.error("Blurt couldn't load its settings", { description: String(e) }));
    void loadLibrary();
    void useDownloads.getState().load();
    return () => {
      cancelled = true;
      unsubscribe?.();
    };
  }, [loadSettings, loadLibrary]);

  // In-app keys: Esc stops everything; Ctrl/⌘+K or "/" jumps to search.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const typing = target.matches("input, textarea, [contenteditable=true]");
      if (e.key === "Escape" && !typing && !document.querySelector("[role=dialog], [role=alertdialog], [role=menu]")) {
        void stopAll();
      } else if ((e.key === "k" && (e.ctrlKey || e.metaKey)) || (e.key === "/" && !typing)) {
        e.preventDefault();
        if (!["all", "favorites", "recent", "downloaded", "category", "collection"].includes(useApp.getState().view.kind)) {
          useApp.getState().setView({ kind: "all" });
        }
        requestAnimationFrame(() => document.getElementById("sound-search")?.focus());
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const onDragStart = (e: DragStartEvent) => {
    const d = e.active.data.current;
    setDragLabel(d?.type === "sound" ? (d.name as string) : null);
  };

  const onDragEnd = ({ active, over }: DragEndEvent) => {
    setDragLabel(null);
    if (!over || active.id === over.id) return;
    const a = active.data.current;
    const o = over.data.current;
    const lib = useLibrary.getState();
    if (a?.type === "sound" && o?.type === "group") {
      const g = o.group as Group;
      if (g.kind === "category") {
        void lib.move([a.soundId], g.id);
        toast.success(`Moved to ${g.name}`);
      } else {
        void lib.setMembership(g.id, [a.soundId], true);
        toast.success(`Added to ${g.name}`);
      }
    } else if (a?.type === "sound" && o?.type === "sound") {
      const { view, settings } = useApp.getState();
      if (!canReorder(view, lib.query, settings?.sort ?? "manual")) return;
      const ordered = [...lib.sounds].sort((x, y) => x.position - y.position).map((s) => s.id);
      const from = ordered.indexOf(a.soundId);
      const to = ordered.indexOf(o.soundId as string);
      if (from >= 0 && to >= 0) void lib.reorder(arrayMove(ordered, from, to));
    } else if (a?.type === "group" && o?.type === "group") {
      const ga = a.group as Group;
      const go = o.group as Group;
      if (ga.kind !== go.kind) return;
      const ids = lib.groups.filter((g) => g.kind === ga.kind).sort((x, y) => x.position - y.position).map((g) => g.id);
      void lib.reorderGroups(arrayMove(ids, ids.indexOf(ga.id), ids.indexOf(go.id)));
    }
  };

  const main =
    view.kind === "settings" ? <SettingsView /> : view.kind === "downloads" ? <DownloadsView /> : <LibraryView />;

  return (
    <TooltipProvider delayDuration={400}>
      <DndContext sensors={sensors} collisionDetection={collision} onDragStart={onDragStart} onDragEnd={onDragEnd} onDragCancel={() => setDragLabel(null)}>
        <div className="flex h-full flex-col">
          <div className="flex min-h-0 flex-1">
            <Sidebar />
            <main className="min-w-0 flex-1 bg-background">{settings ? main : null}</main>
          </div>
          <PlayerBar />
        </div>
        <DragOverlay dropAnimation={null}>
          {dragLabel && (
            <div className="flex w-fit items-center gap-2 rounded-lg border bg-popover px-3 py-2 text-sm font-medium shadow-xl">
              <Music2 className="size-4 text-primary" /> {dragLabel}
            </div>
          )}
        </DragOverlay>
      </DndContext>
      <SoundEditor />
      <ImportDialog />
      <Onboarding />
      <ConfirmHost />
      <DropOverlay visible={hoveringFiles} />
      <Toaster theme={theme === "system" ? "system" : theme} position="bottom-right" offset={{ bottom: 92, right: 16 }} richColors closeButton />
    </TooltipProvider>
  );
}

/** VB-CABLE sometimes only appears after a restart: finish the one-click setup on the next launch. */
async function finishSetupAfterRestart() {
  if (!localStorage.getItem(FINISH_SETUP_KEY)) return;
  localStorage.removeItem(FINISH_SETUP_KEY);
  try {
    const r = await autoSetup();
    toast.success("Your virtual microphone is ready", { description: `Choose “${r.capture}” as your microphone in Discord, Zoom or your game.`, duration: 15000 });
  } catch (e) {
    toast.error("Couldn't finish microphone setup", { description: String(e) });
  }
}
