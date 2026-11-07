import { useRef } from "react";

import { cn } from "@/lib/utils";

/** Decorative waveform used as a generated cover when a sound has no image. */
export function WaveArt({ peaks, color, className }: { peaks: number[]; color: string; className?: string }) {
  const bars = downsample(peaks, 40);
  return (
    <svg viewBox="0 0 40 20" preserveAspectRatio="none" className={cn("h-full w-full", className)} aria-hidden="true">
      {bars.map((p, i) => {
        const h = Math.max(0.6, (p / 255) * 18);
        return <rect key={i} x={i + 0.2} y={10 - h / 2} width={0.6} height={h} rx={0.3} fill={color} />;
      })}
    </svg>
  );
}

function downsample(peaks: number[], n: number): number[] {
  if (peaks.length === 0) return new Array(n).fill(20);
  return Array.from({ length: n }, (_, i) => {
    const a = Math.floor((i * peaks.length) / n);
    const b = Math.max(a + 1, Math.floor(((i + 1) * peaks.length) / n));
    return Math.max(...peaks.slice(a, b));
  });
}

interface TrimProps {
  peaks: number[];
  durationMs: number;
  startMs: number;
  endMs: number;
  onChange: (startMs: number, endMs: number) => void;
}

const MIN_MS = 50;

/** Waveform with two draggable handles (also keyboard-adjustable: arrows = 100 ms, Shift = 1 s). */
export function TrimWaveform({ peaks, durationMs, startMs, endMs, onChange }: TrimProps) {
  const ref = useRef<HTMLDivElement>(null);
  const pct = (ms: number) => (durationMs > 0 ? (ms / durationMs) * 100 : 0);

  const drag = (which: "start" | "end") => (e: React.PointerEvent) => {
    e.preventDefault();
    const el = ref.current;
    if (!el) return;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const r = el.getBoundingClientRect();
      const ms = Math.round(Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width)) * durationMs);
      if (which === "start") onChange(Math.min(ms, endMs - MIN_MS), endMs);
      else onChange(startMs, Math.max(ms, startMs + MIN_MS));
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const key = (which: "start" | "end") => (e: React.KeyboardEvent) => {
    const step = e.shiftKey ? 1000 : 100;
    const d = e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0;
    if (!d) return;
    e.preventDefault();
    if (which === "start") onChange(Math.max(0, Math.min(startMs + d, endMs - MIN_MS)), endMs);
    else onChange(startMs, Math.min(durationMs, Math.max(endMs + d, startMs + MIN_MS)));
  };

  const bars = downsample(peaks, 96);
  return (
    <div ref={ref} className="relative h-24 select-none rounded-lg bg-surface-2">
      <svg viewBox="0 0 96 20" preserveAspectRatio="none" className="absolute inset-0 h-full w-full" aria-hidden="true">
        {bars.map((p, i) => {
          const h = Math.max(0.5, (p / 255) * 17);
          const inside = (i / 96) * 100 >= pct(startMs) && (i / 96) * 100 <= pct(endMs);
          return (
            <rect
              key={i}
              x={i + 0.15}
              y={10 - h / 2}
              width={0.7}
              height={h}
              rx={0.3}
              className={inside ? "fill-primary" : "fill-muted-foreground/30"}
            />
          );
        })}
      </svg>
      <div className="absolute inset-y-0 left-0 bg-background/50" style={{ width: `${pct(startMs)}%` }} />
      <div className="absolute inset-y-0 right-0 bg-background/50" style={{ width: `${100 - pct(endMs)}%` }} />
      {(["start", "end"] as const).map((which) => {
        const ms = which === "start" ? startMs : endMs;
        return (
          <div
            key={which}
            role="slider"
            tabIndex={0}
            aria-label={which === "start" ? "Trim start" : "Trim end"}
            aria-valuemin={0}
            aria-valuemax={durationMs}
            aria-valuenow={ms}
            aria-valuetext={`${(ms / 1000).toFixed(2)} seconds`}
            onPointerDown={drag(which)}
            onKeyDown={key(which)}
            className="absolute inset-y-0 -ml-2 flex w-4 cursor-ew-resize justify-center outline-none focus-visible:[&>span]:ring-2 focus-visible:[&>span]:ring-ring"
            style={{ left: `${pct(ms)}%` }}
          >
            <span className="h-full w-1 rounded-full bg-primary" />
          </div>
        );
      })}
    </div>
  );
}
