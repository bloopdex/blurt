import { memo } from "react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Headphones, Pin, Play, Square, Star } from "lucide-react";

import type { Group, Sound, Voice } from "@/lib/api";
import { effectiveDuration } from "@/lib/filter";
import { formatDuration } from "@/lib/format";
import { cn } from "@/lib/utils";
import { playSound } from "@/lib/playback";
import { useLibrary } from "@/store/library";
import { Kbd } from "@/components/Kbd";
import { WaveArt } from "./Waveform";
import { SoundMenu } from "./SoundMenu";
import type { Settings } from "@/lib/api";

interface Props {
  sound: Sound;
  category: Group | undefined;
  voice: Voice | undefined;
  density: Settings["gridDensity"];
  coversDir: string;
  sortable: boolean;
}

export function EqBars({ className }: { className?: string }) {
  return (
    <span className={cn("flex h-3 items-end gap-[2px]", className)} aria-hidden="true">
      {[0, 0.25, 0.5].map((d) => (
        <span key={d} className="eq-bar h-full w-[3px] rounded-full bg-current" style={{ animationDelay: `${d}s` }} />
      ))}
    </span>
  );
}

export const SoundCard = memo(function SoundCard({ sound, category, voice, density, coversDir, sortable }: Props) {
  const update = useLibrary((s) => s.update);
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: `sound:${sound.id}`,
    data: { type: "sound", soundId: sound.id, name: sound.name },
  });
  const style = { transform: sortable ? CSS.Translate.toString(transform) : undefined, transition };
  const playing = !!voice;
  const progress = voice && voice.durationMs > 0 ? Math.min(100, (voice.positionMs / voice.durationMs) * 100) : 0;
  const tint = category?.color ?? "#8A8F9C";
  const cover = sound.coverFile ? convertFileSrc(`${coversDir}/${sound.coverFile}`) : null;
  const label = `${playing ? "Stop or restart" : "Play"} ${sound.name}${sound.shortcut ? `, shortcut ${sound.shortcut}` : ""}`;

  const favorite = (
    <button
      type="button"
      onClick={() => void update(sound.id, { isFavorite: !sound.isFavorite })}
      aria-label={sound.isFavorite ? `Remove ${sound.name} from favorites` : `Add ${sound.name} to favorites`}
      aria-pressed={sound.isFavorite}
      className={cn(
        "grid size-7 shrink-0 place-items-center rounded-md transition-colors hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
        sound.isFavorite ? "text-warn" : "text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
      )}
    >
      <Star className={cn("size-4", sound.isFavorite && "fill-current")} />
    </button>
  );

  const meta = (
    <div className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground tabular-nums">
      {playing ? <EqBars className="text-primary" /> : null}
      <span>{formatDuration(effectiveDuration(sound))}</span>
      {sound.isPinned && <Pin className="size-3" aria-label="Pinned" />}
      {sound.shortcut && <Kbd shortcut={sound.shortcut} className="ml-auto" />}
    </div>
  );

  if (density === "compact") {
    return (
      <div ref={setNodeRef} style={style} className={cn("group relative", isDragging && "z-10 opacity-40")}>
        <SoundMenu sound={sound}>
          <div
            className={cn(
              "flex h-14 items-center gap-3 rounded-lg border bg-card pr-2 pl-3 transition-colors hover:bg-surface-2",
              playing && "border-primary/60 bg-primary/[0.06]",
            )}
          >
            <span className="h-7 w-1 shrink-0 rounded-full" style={{ background: tint }} />
            <button
              type="button"
              onClick={() => void playSound(sound.id)}
              aria-label={label}
              className="flex min-w-0 flex-1 items-center gap-3 self-stretch text-left outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-md"
              {...attributes}
              {...listeners}
            >
              <span className={cn("grid size-8 shrink-0 place-items-center rounded-full", playing ? "bg-primary text-primary-foreground" : "bg-surface-3")}>
                {playing ? <Square className="size-3.5 fill-current" /> : <Play className="size-3.5 translate-x-px fill-current" />}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium">{sound.name}</span>
                {meta}
              </span>
            </button>
            {favorite}
          </div>
        </SoundMenu>
        {playing && (
          <div className="pointer-events-none absolute inset-x-3 bottom-0 h-0.5 overflow-hidden rounded-full">
            <div className="h-full bg-primary transition-[width] duration-100 ease-linear" style={{ width: `${progress}%` }} />
          </div>
        )}
      </div>
    );
  }

  return (
    <div ref={setNodeRef} style={style} className={cn("group relative", isDragging && "z-10 opacity-40")}>
      <SoundMenu sound={sound}>
        <div
          className={cn(
            "overflow-hidden rounded-xl border bg-card transition-[border-color,box-shadow] hover:border-muted-foreground/30",
            playing && "border-primary/70 shadow-[0_0_0_1px_var(--primary)]",
          )}
        >
          <button
            type="button"
            onClick={() => void playSound(sound.id)}
            aria-label={label}
            className="relative block aspect-[16/10] w-full overflow-hidden outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
            style={{ background: `color-mix(in oklab, ${tint} 14%, var(--surface-2))` }}
            {...attributes}
            {...listeners}
          >
            {cover ? (
              <img src={cover} alt="" className="h-full w-full object-cover" draggable={false} />
            ) : (
              <WaveArt peaks={sound.waveform} color={tint} className="absolute inset-0 px-4 py-6 opacity-80" />
            )}
            <span
              className={cn(
                "absolute inset-0 grid place-items-center bg-black/0 transition-colors group-hover:bg-black/25",
                playing && "bg-black/25",
              )}
            >
              <span
                className={cn(
                  "grid size-12 place-items-center rounded-full bg-primary text-primary-foreground shadow-lg transition-[opacity,transform]",
                  playing ? "opacity-100" : "scale-90 opacity-0 group-hover:scale-100 group-hover:opacity-100 group-focus-within:opacity-100",
                )}
              >
                {playing ? <Square className="size-4 fill-current" /> : <Play className="size-5 translate-x-px fill-current" />}
              </span>
            </span>
            {playing && (
              <span className="absolute inset-x-0 bottom-0 h-1 bg-black/30">
                <span className="block h-full bg-primary transition-[width] duration-100 ease-linear" style={{ width: `${progress}%` }} />
              </span>
            )}
            {voice && !voice.onMic && (
              <span className="absolute top-2 left-2 flex items-center gap-1 rounded-md bg-black/60 px-1.5 py-0.5 text-[11px] text-white">
                <Headphones className="size-3" /> Only you
              </span>
            )}
          </button>
          <div className="flex items-start gap-1 p-3 pr-2">
            <div className="min-w-0 flex-1 space-y-1">
              <div className={cn("truncate font-medium", density === "large" ? "text-[15px]" : "text-sm")}>{sound.name}</div>
              {meta}
            </div>
            {favorite}
          </div>
        </div>
      </SoundMenu>
    </div>
  );
});
