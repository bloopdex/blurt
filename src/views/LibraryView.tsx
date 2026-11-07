import { useMemo } from "react";
import { rectSortingStrategy, SortableContext } from "@dnd-kit/sortable";
import { ArrowDownUp, Clock, Download, FolderOpen, LayoutGrid, Plus, Rows3, Search, Star, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { SoundCard } from "@/components/sounds/SoundCard";
import { pickFilesToImport } from "@/lib/actions";
import { SpeakButton } from "@/components/sounds/SpeakButton";
import type { Settings } from "@/lib/api";
import { canReorder, visibleSounds } from "@/lib/filter";
import { GroupIcon } from "@/lib/icons";
import { cn } from "@/lib/utils";
import { useApp } from "@/store/app";
import { useLibrary } from "@/store/library";

const sortLabels: Record<Settings["sort"], string> = {
  manual: "Custom order",
  name: "Name",
  recent: "Recently added",
  plays: "Most played",
  duration: "Duration",
};

const gridSize: Record<Settings["gridDensity"], string> = {
  compact: "grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-2",
  comfortable: "grid-cols-[repeat(auto-fill,minmax(176px,1fr))] gap-4",
  large: "grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-5",
};

export function LibraryView() {
  const { view, settings, info, voices, updateSettings, setView } = useApp();
  const { sounds, groups, query, setQuery, loaded } = useLibrary();
  const sort = settings?.sort ?? "manual";
  const density = settings?.gridDensity ?? "comfortable";

  const list = useMemo(() => visibleSounds(sounds, view, query, sort), [sounds, view, query, sort]);
  const groupById = useMemo(() => new Map(groups.map((g) => [g.id, g])), [groups]);
  const voiceBySound = useMemo(() => {
    const m = new Map<string, (typeof voices)[number]>();
    for (const v of voices) if (v.soundId && !v.preview) m.set(v.soundId, v);
    return m;
  }, [voices]);
  const sortable = canReorder(view, query, sort);

  const group = "id" in view ? groupById.get(view.id) : undefined;
  const title =
    group?.name ??
    { all: "All sounds", favorites: "Favorites", recent: "Recently played", downloaded: "Downloaded" }[view.kind as string] ??
    "Sounds";
  const subtitle = {
    favorites: "Your quick-access sounds.",
    recent: "The last sounds you played.",
    downloaded: "Sounds you saved from links.",
    collection: "A collection — sounds can belong to several.",
  }[view.kind as string];

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex flex-wrap items-center gap-3 border-b px-6 py-4">
        <div className="mr-auto flex min-w-0 items-center gap-3">
          {group && (
            <span className="grid size-9 place-items-center rounded-lg" style={{ background: `color-mix(in oklab, ${group.color} 18%, transparent)` }}>
              <GroupIcon name={group.icon} color={group.color} className="size-[18px]" />
            </span>
          )}
          <div className="min-w-0">
            <h1 className="truncate font-display text-xl font-bold tracking-tight">{title}</h1>
            <p className="text-xs text-muted-foreground">
              {list.length} {list.length === 1 ? "sound" : "sounds"}
              {subtitle ? ` · ${subtitle}` : ""}
            </p>
          </div>
        </div>
        <div className="relative w-64">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            id="sound-search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && (setQuery(""), e.currentTarget.blur())}
            placeholder="Search sounds"
            aria-label="Search sounds"
            className="h-9 pr-8 pl-8"
          />
          {query && (
            <button
              type="button"
              aria-label="Clear search"
              onClick={() => setQuery("")}
              className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            >
              <X className="size-4" />
            </button>
          )}
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm" className="h-9" disabled={view.kind === "recent"}>
              <ArrowDownUp /> {view.kind === "recent" ? "Last played" : sortLabels[sort]}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-48">
            <DropdownMenuLabel>Sort by</DropdownMenuLabel>
            <DropdownMenuRadioGroup value={sort} onValueChange={(v) => void updateSettings({ sort: v as Settings["sort"] })}>
              {Object.entries(sortLabels).map(([k, label]) => (
                <DropdownMenuRadioItem key={k} value={k}>
                  {label}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
            <DropdownMenuSeparator />
            <DropdownMenuLabel>Card size</DropdownMenuLabel>
            <DropdownMenuRadioGroup
              value={density}
              onValueChange={(v) => void updateSettings({ gridDensity: v as Settings["gridDensity"] })}
            >
              <DropdownMenuRadioItem value="compact">
                <Rows3 className="size-4" /> Compact
              </DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="comfortable">
                <LayoutGrid className="size-4" /> Comfortable
              </DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="large">
                <LayoutGrid className="size-4" /> Large
              </DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>
        <SpeakButton />
        <Button className="h-9" onClick={() => void pickFilesToImport()}>
          <Plus /> Add sounds
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        {!loaded ? null : list.length === 0 ? (
          <EmptyState kind={view.kind} searching={!!query} onClear={() => setQuery("")} onDownload={() => setView({ kind: "downloads" })} />
        ) : (
          <SortableContext
            items={list.map((s) => `sound:${s.id}`)}
            strategy={sortable ? rectSortingStrategy : () => null}
          >
            <div className={cn("grid", gridSize[density])}>
              {list.map((s) => (
                <SoundCard
                  key={s.id}
                  sound={s}
                  category={s.categoryId ? groupById.get(s.categoryId) : undefined}
                  voice={voiceBySound.get(s.id)}
                  density={density}
                  coversDir={info?.coversDir ?? ""}
                  sortable={sortable}
                />
              ))}
            </div>
          </SortableContext>
        )}
      </div>
    </div>
  );
}

function EmptyState({
  kind,
  searching,
  onClear,
  onDownload,
}: {
  kind: string;
  searching: boolean;
  onClear: () => void;
  onDownload: () => void;
}) {
  if (searching) {
    return (
      <div className="grid h-full place-items-center">
        <div className="text-center">
          <p className="font-medium">No sounds match your search</p>
          <Button variant="link" onClick={onClear}>
            Clear search
          </Button>
        </div>
      </div>
    );
  }
  const copy: Record<string, { icon: typeof Star; title: string; body: string }> = {
    favorites: { icon: Star, title: "No favorites yet", body: "Star a sound to keep it one click away." },
    recent: { icon: Clock, title: "Nothing played yet", body: "Sounds you play show up here." },
    downloaded: { icon: Download, title: "No downloaded sounds", body: "Save audio from YouTube, TikTok and more." },
    category: { icon: FolderOpen, title: "This category is empty", body: "Drag sounds onto it in the sidebar, or add new ones." },
    collection: { icon: FolderOpen, title: "This collection is empty", body: "Drag sounds onto it in the sidebar, or use a sound's right-click menu." },
  };
  const c = copy[kind];
  if (c) {
    const Icon = c.icon;
    return (
      <div className="grid h-full place-items-center">
        <div className="max-w-sm text-center">
          <Icon className="mx-auto mb-3 size-8 text-muted-foreground" />
          <p className="font-medium">{c.title}</p>
          <p className="mt-1 text-sm text-muted-foreground">{c.body}</p>
          {kind === "downloaded" && (
            <Button variant="outline" className="mt-4" onClick={onDownload}>
              <Download /> Download a sound
            </Button>
          )}
        </div>
      </div>
    );
  }
  return (
    <div className="grid h-full place-items-center">
      <div className="max-w-md rounded-2xl border border-dashed p-10 text-center">
        <div className="mx-auto mb-4 grid size-12 place-items-center rounded-xl bg-primary/15 text-primary">
          <Plus className="size-6" />
        </div>
        <p className="font-display text-lg font-bold">Your board is empty</p>
        <p className="mt-1 text-sm text-muted-foreground">
          Drop audio files anywhere on this window, pick files from your computer, or grab a clip from a link.
        </p>
        <div className="mt-5 flex justify-center gap-2">
          <Button onClick={() => void pickFilesToImport()}>
            <Plus /> Add sounds
          </Button>
          <Button variant="outline" onClick={onDownload}>
            <Download /> From a link
          </Button>
        </div>
      </div>
    </div>
  );
}
