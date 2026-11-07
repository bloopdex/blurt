import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { Group } from "@/lib/api";
import { GroupIcon, groupColors, groupIcons } from "@/lib/icons";
import { cn } from "@/lib/utils";
import { useApp } from "@/store/app";
import { useLibrary } from "@/store/library";

export function GroupDialog({ kind, group, onClose }: { kind: Group["kind"]; group: Group | null; onClose: () => void }) {
  const { createGroup, updateGroup } = useLibrary();
  const setView = useApp((s) => s.setView);
  const [name, setName] = useState(group?.name ?? "");
  const [icon, setIcon] = useState(group?.icon ?? (kind === "collection" ? "star" : "folder"));
  const [color, setColor] = useState(group?.color ?? groupColors[0]);
  const [busy, setBusy] = useState(false);
  const valid = name.trim().length > 0 && name.trim().length <= 60;

  const save = async () => {
    if (!valid) return;
    setBusy(true);
    if (group) {
      if (await updateGroup(group.id, name, icon, color)) onClose();
    } else {
      const g = await createGroup(kind, name, icon, color);
      if (g) {
        setView({ kind, id: g.id });
        onClose();
      }
    }
    setBusy(false);
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{group ? `Edit ${kind}` : `New ${kind}`}</DialogTitle>
          <DialogDescription>
            {kind === "category"
              ? "Each sound lives in one category. Drag sounds onto it in the sidebar."
              : "Collections are playlists for a moment — a sound can be in several."}
          </DialogDescription>
        </DialogHeader>
        <form
          className="space-y-5"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <div className="space-y-2">
            <Label htmlFor="group-name">Name</Label>
            <div className="flex items-center gap-2">
              <span className="grid size-9 shrink-0 place-items-center rounded-md bg-surface-2">
                <GroupIcon name={icon} color={color} className="size-4" />
              </span>
              <Input
                id="group-name"
                autoFocus
                value={name}
                maxLength={60}
                placeholder={kind === "category" ? "e.g. Reactions" : "e.g. Friday stream"}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
          </div>
          <fieldset className="space-y-2">
            <legend className="text-sm font-medium">Icon</legend>
            <div className="grid grid-cols-8 gap-1">
              {Object.keys(groupIcons).map((k) => (
                <button
                  key={k}
                  type="button"
                  aria-label={k}
                  aria-pressed={icon === k}
                  onClick={() => setIcon(k)}
                  className={cn(
                    "grid size-9 place-items-center rounded-md text-muted-foreground hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                    icon === k && "bg-accent text-foreground ring-1 ring-ring",
                  )}
                >
                  <GroupIcon name={k} className="size-4" />
                </button>
              ))}
            </div>
          </fieldset>
          <fieldset className="space-y-2">
            <legend className="text-sm font-medium">Colour</legend>
            <div className="flex gap-2">
              {groupColors.map((c) => (
                <button
                  key={c}
                  type="button"
                  aria-label={`Colour ${c}`}
                  aria-pressed={color === c}
                  onClick={() => setColor(c)}
                  className={cn(
                    "size-7 rounded-full ring-offset-2 ring-offset-background focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                    color === c && "ring-2 ring-foreground",
                  )}
                  style={{ background: c }}
                />
              ))}
            </div>
          </fieldset>
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={!valid || busy}>
              {group ? "Save" : `Create ${kind}`}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
