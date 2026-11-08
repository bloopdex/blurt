import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Mic, MicOff, Pin, RotateCcw, Square, Star, Volume2, VolumeX } from "lucide-react";

import { LogoMark } from "@/components/Logo";
import { EqBars } from "@/components/sounds/SoundCard";
import { Slider } from "@/components/ui/slider";
import { api, errorMessage, type PlayOutcome, type Settings, type Sound } from "@/lib/api";
import { cn } from "@/lib/utils";

const KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
/** Backend key codes after 1–10: volume down, volume up, mic mute (see OVERLAY_KEYS in shortcuts.rs). */
const VOLUME_UP = 12, MIC_MUTE = 13; // 11 = volume down

type Live = { soundId: string | null; onMic: boolean; positionMs: number; durationMs: number };
/** Scaled-down BGRA snapshot of the screen behind the overlay (see overlay.rs), shown blurred. */
type Backdrop = { width: number; height: number; bgra: number[] };

function paintBackdrop(canvas: HTMLCanvasElement | null, b: Backdrop | null) {
  if (!canvas || !b) return;
  canvas.width = b.width;
  canvas.height = b.height;
  const img = new ImageData(b.width, b.height);
  for (let i = 0; i < b.bgra.length; i += 4) {
    img.data[i] = b.bgra[i + 2];
    img.data[i + 1] = b.bgra[i + 1];
    img.data[i + 2] = b.bgra[i];
    img.data[i + 3] = 255;
  }
  canvas.getContext("2d")?.putImageData(img, 0, 0);
}

/** In-game picker. It never takes focus (the game stays in front); keys arrive from the backend. */
export function Overlay() {
  const [sounds, setSounds] = useState<Sound[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [note, setNote] = useState<{ text: string; bad: boolean } | null>(null);
  const [live, setLive] = useState<Live[]>([]);
  const backdrop = useRef<HTMLCanvasElement>(null);
  // The idle close timer lives in Rust (WebView2 throttles timers here); this just restarts it.
  const keepOpen = () => void api.overlayActivity();

  const report = (name: string, r: Promise<PlayOutcome>) =>
    r.then(
      (o) => setNote(o.warning ? { text: o.warning, bad: true } : { text: `Playing ${name}`, bad: false }),
      (e) => setNote({ text: errorMessage(e), bad: true }),
    );
  const play = (s: Sound | undefined) => {
    keepOpen();
    if (s) void report(s.name, api.playSound(s.id));
  };
  const update = (patch: Partial<Settings>) => {
    keepOpen();
    api.updateSettings(patch).then(setSettings, (e) => setNote({ text: errorMessage(e), bad: true }));
  };

  // Listeners are registered once, so they read the latest state through refs.
  const ref = useRef({ sounds, settings, play, update });
  ref.current = { sounds, settings, play, update };

  useEffect(() => {
    document.documentElement.classList.add("dark");
    document.body.style.background = "transparent";
    const load = (b: Backdrop | null = null) => {
      paintBackdrop(backdrop.current, b);
      setNote(null);
      api.getSettings().then(setSettings, () => {});
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
    const onKey = (n: number) => {
      const { sounds, settings, play, update } = ref.current;
      if (n <= KEYS.length) return play(sounds[n - 1]);
      if (!settings) return;
      if (n === MIC_MUTE) return update({ micMuted: !settings.micMuted });
      const step = n === VOLUME_UP ? 0.1 : -0.1;
      update({ masterVolume: Math.min(2, Math.max(0, Math.round((settings.masterVolume + step) * 10) / 10)) });
    };
    load();
    const offs = [
      listen<Backdrop | null>("overlay-shown", (e) => load(e.payload)),
      listen<number>("overlay-key", (e) => onKey(e.payload)),
      listen<Settings>("settings-changed", (e) => setSettings(e.payload)),
      listen<Live[]>("voices", (e) => setLive(e.payload.filter((v) => v.onMic))),
    ];
    return () => offs.forEach((o) => void o.then((u) => u()));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const progress = (id: string) => {
    const v = live.find((x) => x.soundId === id);
    return v ? Math.min(1, v.positionMs / Math.max(1, v.durationMs)) : null;
  };
  const volume = Math.round((settings?.masterVolume ?? 1) * 100);
  const micVolume = Math.round((settings?.micVolume ?? 1) * 100);
  const micOn = !!settings?.micPassthrough;
  const muted = !!settings?.micMuted;

  return (
    <div className="relative flex h-full flex-col gap-3 overflow-hidden rounded-lg border border-white/10 bg-[#0d0e11] p-4 text-foreground shadow-2xl">
      {/* Frosted glass: what's behind the overlay, blurred, under a dark tint. */}
      <canvas ref={backdrop} aria-hidden className="pointer-events-none absolute inset-0 size-full scale-125 blur-2xl saturate-150" />
      <div aria-hidden className="pointer-events-none absolute inset-0 bg-[#0d0e11]/60" />
      {/* Soft brand glow on top of the frosted window. */}
      <div aria-hidden className="pointer-events-none absolute -top-24 -left-16 size-72 rounded-full bg-primary/25 blur-3xl" />
      <div aria-hidden className="pointer-events-none absolute -right-20 -bottom-28 size-72 rounded-full bg-violet-500/20 blur-3xl" />

      <header className="relative flex items-center gap-2">
        <LogoMark className="size-6" />
        <span className="font-display text-base font-extrabold tracking-tight">blurt</span>
        <span className="ml-2 rounded-full bg-white/[0.06] px-2.5 py-1 text-[11px] text-white/60">1–9 · 0 to play · closes by itself</span>
        <div className="ml-auto flex gap-2">
          <button type="button" onClick={() => (keepOpen(), void report("last sound", api.replayLast()))} className="glass-btn">
            <RotateCcw className="size-4" /> Replay
          </button>
          <button type="button" onClick={() => (keepOpen(), void api.stopAll())} className="glass-btn">
            <Square className="size-4" /> Stop
          </button>
        </div>
      </header>

      <div className="relative grid min-h-0 flex-1 grid-cols-5 grid-rows-2 gap-2">
        {sounds.map((s, i) => {
          const p = progress(s.id);
          return (
            <button
              key={s.id}
              type="button"
              onClick={() => play(s)}
              className={cn(
                "group relative flex flex-col justify-between overflow-hidden rounded-2xl border border-white/[0.07] bg-white/[0.045] p-3 text-left transition-colors hover:border-primary/50 hover:bg-white/[0.08]",
                p !== null && "border-primary/80 bg-primary/15 shadow-[0_0_28px_-6px] shadow-primary/60",
              )}
            >
              <span className="flex items-center justify-between">
                <span className="grid size-8 place-items-center rounded-lg bg-primary/20 font-display text-lg font-bold text-primary">{KEYS[i]}</span>
                {p !== null && <EqBars className="h-4 text-primary" />}
              </span>
              <span className="flex items-center gap-1 text-sm leading-tight font-semibold">
                {s.isPinned && <Pin className="size-3 shrink-0 text-primary" />}
                {s.isFavorite && <Star className="size-3 shrink-0 fill-current text-primary" />}
                <span className="line-clamp-2">{s.name}</span>
              </span>
              {p !== null && <span className="absolute inset-x-0 bottom-0 h-1 origin-left bg-primary" style={{ transform: `scaleX(${p})` }} />}
            </button>
          );
        })}
        {sounds.length === 0 && <p className="col-span-5 row-span-2 grid place-items-center text-sm text-white/60">Your library is empty.</p>}
      </div>

      <footer className="relative flex items-center gap-4 rounded-2xl border border-white/[0.07] bg-black/25 px-4 py-2.5">
        <div className="flex flex-1 items-center gap-3">
          <button type="button" aria-label={volume ? "Mute sounds" : "Unmute sounds"} onClick={() => update({ masterVolume: volume ? 0 : 0.8 })} className="text-white/80 hover:text-white">
            {volume ? <Volume2 className="size-4" /> : <VolumeX className="size-4 text-warn" />}
          </button>
          <span className="w-12 text-xs text-white/60">Sounds</span>
          <Slider aria-label="Sound volume" min={0} max={200} step={5} value={[volume]} onValueChange={([v]) => update({ masterVolume: v / 100 })} className="flex-1" />
          <span className="w-10 text-right text-xs text-white/70 tabular-nums">{volume}%</span>
        </div>
        <div className="h-6 w-px bg-white/10" />
        <div className={cn("flex flex-1 items-center gap-3", !micOn && "opacity-50")}>
          <button
            type="button"
            disabled={!micOn}
            aria-label={muted ? "Unmute my mic" : "Mute my mic"}
            onClick={() => update({ micMuted: !muted })}
            className={cn("rounded-md p-1", muted ? "bg-destructive/20 text-destructive" : "text-white/80 hover:text-white")}
          >
            {muted ? <MicOff className="size-4" /> : <Mic className="size-4" />}
          </button>
          <span className="w-12 text-xs text-white/60">{micOn ? (muted ? "Muted" : "My mic") : "Mic off"}</span>
          <Slider aria-label="Mic volume" disabled={!micOn || muted} min={0} max={200} step={5} value={[micVolume]} onValueChange={([v]) => update({ micVolume: v / 100 })} className="flex-1" />
          <span className="w-10 text-right text-xs text-white/70 tabular-nums">{micVolume}%</span>
        </div>
        <span className="hidden text-[11px] text-white/45 sm:block">− = volume · M mic</span>
      </footer>

      {note && <p className={cn("relative -mt-1 truncate px-1 text-xs", note.bad ? "text-warn" : "text-white/60")}>{note.text}</p>}
    </div>
  );
}
