import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Pin, RotateCcw, Square, Star } from "lucide-react";

import { api, errorMessage, type PlayOutcome, type Sound } from "@/lib/api";
import { cn } from "@/lib/utils";

const KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
/** Esc can't be relied on (games like Valorant keep it), so the overlay closes itself when idle. */
const IDLE_CLOSE_MS = 4000;

/** In-game picker. It never takes focus (the game stays in front); keys 1–9 and 0 arrive from the backend. */
export function Overlay() {
  const [sounds, setSounds] = useState<Sound[]>([]);
  const [note, setNote] = useState<{ text: string; bad: boolean } | null>(null);
  const [playing, setPlaying] = useState<string[]>([]);
  const idle = useRef<number | undefined>(undefined);
  const keepOpen = () => {
    clearTimeout(idle.current);
    idle.current = window.setTimeout(() => void api.hideOverlay(), IDLE_CLOSE_MS);
  };

  const report = (name: string, r: Promise<PlayOutcome>) =>
    r.then(
      (o) => setNote(o.warning ? { text: o.warning, bad: true } : { text: `Playing ${name}`, bad: false }),
      (e) => setNote({ text: errorMessage(e), bad: true }),
    );
  const play = (s: Sound | undefined) => {
    keepOpen();
    if (s) void report(s.name, api.playSound(s.id));
  };

  const soundsRef = useRef(sounds);
  soundsRef.current = sounds;

  useEffect(() => {
    document.documentElement.classList.add("dark");
    document.body.style.background = "transparent";
    const load = () => {
      setNote(null);
      keepOpen();
      api.getLibrary().then(
        (l) =>
          setSounds(
            l.sounds
              .sort(
                (a, b) =>
                  Number(b.isPinned) - Number(a.isPinned) ||
                  (a.isPinned && b.isPinned ? a.position - b.position : 0) ||
                  Number(b.isFavorite) - Number(a.isFavorite) ||
                  b.playCount - a.playCount ||
                  a.name.localeCompare(b.name),
              )
              .slice(0, KEYS.length),
          ),
        (e) => setNote({ text: errorMessage(e), bad: true }),
      );
    };
    load();
    const offs = [
      listen("overlay-shown", load),
      listen<number>("overlay-key", (e) => play(soundsRef.current[e.payload - 1])),
      listen<{ soundId: string | null; onMic: boolean }[]>("voices", (e) => setPlaying(e.payload.filter((v) => v.onMic).flatMap((v) => (v.soundId ? [v.soundId] : [])))),
    ];
    return () => offs.forEach((o) => void o.then((u) => u()));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="flex h-full flex-col gap-3 rounded-2xl border border-white/10 bg-background/90 p-4 text-foreground backdrop-blur">
      <div className="flex items-center gap-2">
        <p className="flex-1 text-sm text-muted-foreground">Press 1–9 or 0 to play · closes by itself</p>
        <button type="button" onClick={() => (keepOpen(), void report("last sound", api.replayLast()))} className="flex h-9 items-center gap-1.5 rounded-lg bg-surface-2 px-3 text-sm hover:bg-surface-3">
          <RotateCcw className="size-4" /> Replay
        </button>
        <button type="button" onClick={() => void api.stopAll()} className="flex h-9 items-center gap-1.5 rounded-lg bg-surface-2 px-3 text-sm hover:bg-surface-3">
          <Square className="size-4" /> Stop
        </button>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-5 grid-rows-2 gap-2">
        {sounds.map((s, i) => (
          <button
            key={s.id}
            type="button"
            onClick={() => play(s)}
            className={cn(
              "flex flex-col justify-between rounded-lg border border-white/5 bg-surface-2 p-3 text-left hover:border-primary/60 hover:bg-surface-3",
              playing.includes(s.id) && "border-primary bg-primary/15",
            )}
          >
            <span className="grid size-8 place-items-center rounded-md bg-primary/20 font-display text-lg font-bold text-primary">{KEYS[i]}</span>
            <span className="flex items-center gap-1 text-sm font-medium">
              {s.isPinned && <Pin className="size-3 shrink-0 text-primary" />}
              {s.isFavorite && <Star className="size-3 shrink-0 fill-current text-primary" />}
              <span className="line-clamp-2">{s.name}</span>
            </span>
          </button>
        ))}
        {sounds.length === 0 && <p className="col-span-5 row-span-2 grid place-items-center text-sm text-muted-foreground">Your library is empty.</p>}
      </div>
      {note && <p className={cn("truncate text-xs", note.bad ? "text-warn" : "text-muted-foreground")}>{note.text}</p>}
    </div>
  );
}
