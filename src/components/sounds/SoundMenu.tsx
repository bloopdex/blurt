import type { ReactNode } from "react";
import { Copy, FolderInput, Headphones, ImagePlus, Library, Pencil, Pin, Play, Star, Trash2 } from "lucide-react";

import {
  ContextMenu,
  ContextMenuCheckboxItem,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { api, type Sound } from "@/lib/api";
import { deleteSounds, pickCover } from "@/lib/actions";
import { GroupIcon } from "@/lib/icons";
import { playSound, preview } from "@/lib/playback";
import { useLibrary } from "@/store/library";

export function SoundMenu({ sound, children }: { sound: Sound; children: ReactNode }) {
  const { groups, update, move, setMembership, duplicate, setEditingId } = useLibrary();
  const categories = groups.filter((g) => g.kind === "category").sort((a, b) => a.position - b.position);
  const collections = groups.filter((g) => g.kind === "collection").sort((a, b) => a.position - b.position);

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-60">
        <ContextMenuItem onSelect={() => void playSound(sound.id)}>
          <Play /> Play
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={() =>
            void preview(() => api.previewSound(sound.id, sound.trimStartMs, sound.trimEndMs, sound.volume))
          }
        >
          <Headphones /> Preview on my speakers
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={() => setEditingId(sound.id)}>
          <Pencil /> Edit…
          <ContextMenuShortcut>Trim, shortcut</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => void update(sound.id, { isFavorite: !sound.isFavorite })}>
          <Star /> {sound.isFavorite ? "Remove from favorites" : "Add to favorites"}
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => void update(sound.id, { isPinned: !sound.isPinned })}>
          <Pin /> {sound.isPinned ? "Unpin" : "Pin to top"}
        </ContextMenuItem>
        <ContextMenuSub>
          <ContextMenuSubTrigger>
            <FolderInput /> Move to category
          </ContextMenuSubTrigger>
          <ContextMenuSubContent className="w-52">
            <ContextMenuCheckboxItem
              checked={sound.categoryId === null}
              onSelect={() => void move([sound.id], null)}
            >
              No category
            </ContextMenuCheckboxItem>
            {categories.map((c) => (
              <ContextMenuCheckboxItem
                key={c.id}
                checked={sound.categoryId === c.id}
                onSelect={() => void move([sound.id], c.id)}
              >
                <GroupIcon name={c.icon} color={c.color} className="size-4" /> {c.name}
              </ContextMenuCheckboxItem>
            ))}
          </ContextMenuSubContent>
        </ContextMenuSub>
        {collections.length > 0 && (
          <ContextMenuSub>
            <ContextMenuSubTrigger>
              <Library /> Collections
            </ContextMenuSubTrigger>
            <ContextMenuSubContent className="w-52">
              {collections.map((c) => {
                const member = sound.collectionIds.includes(c.id);
                return (
                  <ContextMenuCheckboxItem
                    key={c.id}
                    checked={member}
                    onSelect={(e) => {
                      e.preventDefault();
                      void setMembership(c.id, [sound.id], !member);
                    }}
                  >
                    <GroupIcon name={c.icon} color={c.color} className="size-4" /> {c.name}
                  </ContextMenuCheckboxItem>
                );
              })}
            </ContextMenuSubContent>
          </ContextMenuSub>
        )}
        <ContextMenuItem onSelect={() => void pickCover(sound.id)}>
          <ImagePlus /> Change cover…
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => void duplicate(sound.id)}>
          <Copy /> Duplicate
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem variant="destructive" onSelect={() => void deleteSounds([sound.id])}>
          <Trash2 /> Delete
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
