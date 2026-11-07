import { useMemo, useState } from "react";
import { SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  Clock,
  Download,
  LayoutGrid,
  MoreHorizontal,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Search,
  Settings as SettingsIcon,
  Star,
  type LucideIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { LogoMark, Wordmark } from "@/components/Logo";
import { confirm } from "@/components/ConfirmDialog";
import { GroupDialog } from "@/components/GroupDialog";
import { RoutingPill } from "@/components/routing/RoutingPill";
import type { Group } from "@/lib/api";
import { GroupIcon } from "@/lib/icons";
import { cn } from "@/lib/utils";
import { useApp, type View } from "@/store/app";
import { useDownloads } from "@/store/downloads";
import { useLibrary } from "@/store/library";

function sameView(a: View, b: View) {
  return a.kind === b.kind && ("id" in a ? a.id : null) === ("id" in b ? b.id : null);
}

function NavItem({
  icon: Icon,
  label,
  count,
  active,
  collapsed,
  onClick,
  badge,
}: {
  icon: LucideIcon;
  label: string;
  count?: number;
  active: boolean;
  collapsed: boolean;
  onClick: () => void;
  badge?: number;
}) {
  const btn = (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-sm text-muted-foreground transition-colors outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring",
        active && "bg-accent font-medium text-foreground",
        collapsed && "justify-center px-0",
      )}
    >
      <Icon className="size-4 shrink-0" aria-hidden="true" />
      {!collapsed && <span className="flex-1 truncate text-left">{label}</span>}
      {!collapsed && badge ? (
        <span className="rounded-full bg-primary px-1.5 text-[11px] font-semibold text-primary-foreground">{badge}</span>
      ) : null}
      {!collapsed && count !== undefined && !badge && (
        <span className="text-xs text-muted-foreground tabular-nums">{count}</span>
      )}
    </button>
  );
  if (!collapsed) return btn;
  return (
    <Tooltip>
      <TooltipTrigger asChild>{btn}</TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

function GroupItem({
  group,
  count,
  active,
  collapsed,
  onEdit,
}: {
  group: Group;
  count: number;
  active: boolean;
  collapsed: boolean;
  onEdit: () => void;
}) {
  const setView = useApp((s) => s.setView);
  const deleteGroup = useLibrary((s) => s.deleteGroup);
  const { attributes, listeners, setNodeRef, transform, transition, isOver, active: dragging } = useSortable({
    id: `group:${group.id}`,
    data: { type: "group", group },
  });
  const soundOver = isOver && dragging?.data.current?.type === "sound";
  const remove = async () => {
    const ok = await confirm({
      title: `Delete “${group.name}”?`,
      description:
        group.kind === "category"
          ? "Its sounds stay in your library without a category."
          : "Its sounds stay in your library; only the collection is removed.",
      confirmLabel: "Delete",
      destructive: true,
    });
    if (ok) {
      await deleteGroup(group.id);
      if (useApp.getState().view.kind === group.kind) setView({ kind: "all" });
    }
  };

  const button = (
    <button
      type="button"
      onClick={() => setView({ kind: group.kind, id: group.id })}
      aria-current={active ? "page" : undefined}
      aria-label={`${group.name}, ${count} sounds`}
      className={cn(
        "flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-sm text-muted-foreground transition-colors outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring",
        active && "bg-accent font-medium text-foreground",
        soundOver && "bg-primary/15 text-foreground ring-1 ring-primary",
        collapsed && "justify-center px-0",
      )}
      {...attributes}
      {...listeners}
    >
      <GroupIcon name={group.icon} color={group.color} className="size-4 shrink-0" />
      {!collapsed && <span className="flex-1 truncate text-left">{group.name}</span>}
      {!collapsed && <span className="text-xs tabular-nums group-hover/item:hidden">{count}</span>}
    </button>
  );

  return (
    <div
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className="group/item relative"
    >
      {collapsed ? (
        <Tooltip>
          <TooltipTrigger asChild>{button}</TooltipTrigger>
          <TooltipContent side="right">{group.name}</TooltipContent>
        </Tooltip>
      ) : (
        button
      )}
      {!collapsed && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              aria-label={`Options for ${group.name}`}
              className="absolute top-1 right-1 hidden size-6 place-items-center rounded text-muted-foreground hover:bg-surface-3 hover:text-foreground focus-visible:grid group-hover/item:grid data-[state=open]:grid"
            >
              <MoreHorizontal className="size-4" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" side="right">
            <DropdownMenuItem onSelect={onEdit}>Rename & style…</DropdownMenuItem>
            <DropdownMenuItem variant="destructive" onSelect={() => void remove()}>
              Delete {group.kind}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </div>
  );
}

function GroupSection({ kind, title, collapsed }: { kind: Group["kind"]; title: string; collapsed: boolean }) {
  const { groups, sounds } = useLibrary();
  const view = useApp((s) => s.view);
  const [editing, setEditing] = useState<Group | "new" | null>(null);
  const list = useMemo(
    () => groups.filter((g) => g.kind === kind).sort((a, b) => a.position - b.position),
    [groups, kind],
  );
  const counts = useMemo(() => {
    const m = new Map<string, number>();
    for (const s of sounds) {
      if (kind === "category" && s.categoryId) m.set(s.categoryId, (m.get(s.categoryId) ?? 0) + 1);
      if (kind === "collection") for (const c of s.collectionIds) m.set(c, (m.get(c) ?? 0) + 1);
    }
    return m;
  }, [sounds, kind]);

  return (
    <div className="space-y-0.5">
      {!collapsed ? (
        <div className="flex h-7 items-center justify-between pr-1 pl-2.5">
          <span className="text-[11px] font-semibold tracking-wider text-muted-foreground uppercase">{title}</span>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`New ${kind}`}
            onClick={() => setEditing("new")}
            className="text-muted-foreground"
          >
            <Plus />
          </Button>
        </div>
      ) : (
        <div className="mx-2 my-2 border-t" />
      )}
      <SortableContext items={list.map((g) => `group:${g.id}`)} strategy={verticalListSortingStrategy}>
        {list.map((g) => (
          <GroupItem
            key={g.id}
            group={g}
            count={counts.get(g.id) ?? 0}
            active={sameView(view, { kind, id: g.id } as View)}
            collapsed={collapsed}
            onEdit={() => setEditing(g)}
          />
        ))}
      </SortableContext>
      {!collapsed && list.length === 0 && (
        <button
          type="button"
          onClick={() => setEditing("new")}
          className="w-full rounded-md px-2.5 py-1.5 text-left text-xs text-muted-foreground hover:bg-accent"
        >
          {kind === "collection" ? "Group sounds for a stream, game or meeting…" : "Create a category…"}
        </button>
      )}
      {editing && (
        <GroupDialog kind={kind} group={editing === "new" ? null : editing} onClose={() => setEditing(null)} />
      )}
    </div>
  );
}

export function Sidebar() {
  const { view, setView, settings, updateSettings } = useApp();
  const sounds = useLibrary((s) => s.sounds);
  const downloading = useDownloads((s) => s.items.filter((d) => d.status === "downloading" || d.status === "ready").length);
  const collapsed = settings?.sidebarCollapsed ?? false;
  const counts = useMemo(
    () => ({
      all: sounds.length,
      favorites: sounds.filter((s) => s.isFavorite).length,
      downloaded: sounds.filter((s) => s.sourceUrl).length,
    }),
    [sounds],
  );
  const go = (v: View) => () => setView(v);

  return (
    <aside
      className={cn(
        "flex h-full shrink-0 flex-col border-r bg-surface transition-[width] duration-200",
        collapsed ? "w-[60px]" : "w-[248px]",
      )}
      aria-label="Library navigation"
    >
      <div className={cn("flex h-14 items-center gap-2 px-4", collapsed && "justify-center px-0")}>
        <LogoMark className="size-7 shrink-0" />
        {!collapsed && <Wordmark className="flex-1" />}
        {!collapsed && (
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Collapse sidebar"
            onClick={() => void updateSettings({ sidebarCollapsed: true })}
            className="text-muted-foreground"
          >
            <PanelLeftClose />
          </Button>
        )}
      </div>

      <div className={cn("px-3 pb-2", collapsed && "px-2")}>
        <NavItem
          icon={Search}
          label="Find a sound  (Ctrl+K)"
          active={false}
          collapsed={collapsed}
          onClick={() => {
            if (!["all", "favorites", "recent", "downloaded", "category", "collection"].includes(view.kind)) setView({ kind: "all" });
            requestAnimationFrame(() => document.getElementById("sound-search")?.focus());
          }}
        />
      </div>

      <nav className={cn("min-h-0 flex-1 space-y-4 overflow-y-auto px-3 pb-3", collapsed && "px-2")}>
        <div className="space-y-0.5">
          <NavItem icon={LayoutGrid} label="All sounds" count={counts.all} active={sameView(view, { kind: "all" })} collapsed={collapsed} onClick={go({ kind: "all" })} />
          <NavItem icon={Star} label="Favorites" count={counts.favorites} active={sameView(view, { kind: "favorites" })} collapsed={collapsed} onClick={go({ kind: "favorites" })} />
          <NavItem icon={Clock} label="Recently played" active={sameView(view, { kind: "recent" })} collapsed={collapsed} onClick={go({ kind: "recent" })} />
          <NavItem icon={Download} label="Downloaded" count={counts.downloaded} active={sameView(view, { kind: "downloaded" })} collapsed={collapsed} onClick={go({ kind: "downloaded" })} />
        </div>
        <GroupSection kind="category" title="Categories" collapsed={collapsed} />
        <GroupSection kind="collection" title="Collections" collapsed={collapsed} />
      </nav>

      <div className={cn("space-y-0.5 border-t p-3", collapsed && "px-2")}>
        <NavItem icon={Download} label="Download sounds" active={view.kind === "downloads"} collapsed={collapsed} onClick={go({ kind: "downloads" })} badge={downloading || undefined} />
        <NavItem icon={SettingsIcon} label="Settings" active={view.kind === "settings"} collapsed={collapsed} onClick={go({ kind: "settings" })} />
        <div className={cn("pt-2", collapsed && "flex justify-center")}>
          <RoutingPill compact={collapsed} />
        </div>
        {collapsed && (
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Expand sidebar"
            onClick={() => void updateSettings({ sidebarCollapsed: false })}
            className="mx-auto mt-1 flex text-muted-foreground"
          >
            <PanelLeftOpen />
          </Button>
        )}
      </div>
    </aside>
  );
}
