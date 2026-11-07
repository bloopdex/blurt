import { useMemo } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Headphones, ListMusic, Pause, Play, Radio, Square, Volume2, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Slider } from "@/components/ui/slider";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { EqBars } from "@/components/sounds/SoundCard";
import { RoutingPill } from "@/components/routing/RoutingPill";
import { WaveArt } from "@/components/sounds/Waveform";
import { api, type Voice } from "@/lib/api";
import { formatClock } from "@/lib/format";
import { stopAll } from "@/lib/playback";
import { cn } from "@/lib/utils";
import { currentCheck, useApp } from "@/store/app";
import { useLibrary } from "@/store/library";

function VolumeControl({
  icon: Icon,
  label,
  value,
  onChange,
  disabled,
}: {
  icon: typeof Volume2;
  label: string;
  value: number;
  onChange: (v: number) => void;
  disabled?: boolean;
}) {
  return (
    <div className={cn("flex items-center gap-2", disabled && "opacity-50")}>
      <Tooltip>
        <TooltipTrigger asChild>
          <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        </TooltipTrigger>
        <TooltipContent>{label}</TooltipContent>
      </Tooltip>
      <Slider
        aria-label={label}
        className="w-24"
        min={0}
        max={150}
        step={1}
        value={[Math.round(value * 100)]}
        disabled={disabled}
        onValueChange={([v]) => onChange(v / 100)}
      />
    </div>
  );
}

function VoiceRow({ v }: { v: Voice }) {
  return (
    <li className="flex items-center gap-3 py-2">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-sm">
          <span className="truncate font-medium">{v.name}</span>
          {v.preview ? (
            <span className="text-[11px] text-muted-foreground">preview</span>
          ) : v.onMic ? (
            <span className="text-[11px] text-live">on mic</span>
          ) : (
            <span className="text-[11px] text-warn">only you</span>
          )}
        </div>
        <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-surface-3">
          <div className="h-full bg-primary transition-[width] duration-100 ease-linear" style={{ width: `${v.durationMs ? (v.positionMs / v.durationMs) * 100 : 0}%` }} />
        </div>
      </div>
      <Button variant="ghost" size="icon-xs" aria-label={v.paused ? `Resume ${v.name}` : `Pause ${v.name}`} onClick={() => void api.setPaused(v.voiceId, !v.paused)}>
        {v.paused ? <Play /> : <Pause />}
      </Button>
      <Button variant="ghost" size="icon-xs" aria-label={`Stop ${v.name}`} onClick={() => void api.stopVoice(v.voiceId)}>
        <X />
      </Button>
    </li>
  );
}

export function PlayerBar() {
  const app = useApp();
  const { voices, settings, info, updateSettings } = app;
  // Sent to the device, but a test proved that device doesn't reach a microphone.
  const notReaching = currentCheck(app)?.signalReceived === false;
  const { sounds, groups } = useLibrary();
  const current = voices.at(-1);
  const sound = useMemo(() => sounds.find((s) => s.id === current?.soundId), [sounds, current?.soundId]);
  const category = groups.find((g) => g.id === sound?.categoryId);
  const allPaused = voices.length > 0 && voices.every((v) => v.paused);
  const progress = current && current.durationMs > 0 ? (current.positionMs / current.durationMs) * 100 : 0;

  return (
    <footer className="flex h-[76px] shrink-0 items-center gap-6 border-t bg-surface px-4" aria-label="Playback">
      <div className="flex w-[300px] min-w-0 items-center gap-3">
        <div
          className="relative grid size-12 shrink-0 place-items-center overflow-hidden rounded-lg bg-surface-2"
          style={category ? { background: `color-mix(in oklab, ${category.color} 16%, var(--surface-2))` } : undefined}
        >
          {sound?.coverFile && info ? (
            <img src={convertFileSrc(`${info.coversDir}/${sound.coverFile}`)} alt="" className="h-full w-full object-cover" />
          ) : sound ? (
            <WaveArt peaks={sound.waveform} color={category?.color ?? "#8A8F9C"} className="px-1.5 py-3" />
          ) : (
            <Radio className="size-5 text-muted-foreground" />
          )}
        </div>
        <div className="min-w-0" aria-live="polite">
          {current ? (
            <>
              <div className="flex items-center gap-2">
                {!current.paused && <EqBars className="text-primary" />}
                <span className="truncate text-sm font-medium">{current.name}</span>
              </div>
              <div className="text-xs text-muted-foreground">
                {current.preview
                  ? "Preview · your speakers only"
                  : !current.onMic
                    ? "Playing on your speakers only"
                    : notReaching
                      ? "Sent, but not reaching a microphone"
                      : "Playing into your mic"}
              </div>
            </>
          ) : (
            <>
              <div className="text-sm font-medium text-muted-foreground">Nothing playing</div>
              <div className="text-xs text-muted-foreground">Click a sound to blurt it out</div>
            </>
          )}
        </div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col items-center gap-1.5">
        <div className="flex items-center gap-2">
          <Button
            size="icon"
            className="rounded-full"
            aria-label={allPaused ? "Resume all" : "Pause all"}
            disabled={!voices.length}
            onClick={() => void api.setPaused(null, !allPaused)}
          >
            {allPaused || !voices.length ? <Play className="translate-x-px fill-current" /> : <Pause className="fill-current" />}
          </Button>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" className="rounded-full" aria-label="Stop all sounds" disabled={!voices.length} onClick={() => void stopAll()}>
                <Square className="fill-current" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Stop all (Esc)</TooltipContent>
          </Tooltip>
        </div>
        <div className="flex w-full max-w-xl items-center gap-2 text-[11px] text-muted-foreground tabular-nums">
          <span className="w-9 text-right">{current ? formatClock(current.positionMs) : "0:00"}</span>
          <div className="h-1 flex-1 overflow-hidden rounded-full bg-surface-3" role="progressbar" aria-label="Playback progress" aria-valuenow={Math.round(progress)} aria-valuemin={0} aria-valuemax={100}>
            <div className="h-full rounded-full bg-primary transition-[width] duration-100 ease-linear" style={{ width: `${progress}%` }} />
          </div>
          <span className="w-9">{current ? formatClock(current.durationMs) : "0:00"}</span>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-4">
        <Popover>
          <PopoverTrigger asChild>
            <Button variant="ghost" size="sm" className="gap-1.5 text-muted-foreground" aria-label={`${voices.length} sounds playing`}>
              <ListMusic /> <span className="tabular-nums">{voices.length}</span>
            </Button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-80">
            <div className="mb-1 flex items-center justify-between">
              <span className="text-sm font-semibold">Playing now</span>
              {voices.length > 0 && (
                <Button variant="ghost" size="xs" onClick={() => void stopAll()}>
                  Stop all
                </Button>
              )}
            </div>
            {voices.length ? (
              <ul className="divide-y">{[...voices].reverse().map((v) => <VoiceRow key={v.voiceId} v={v} />)}</ul>
            ) : (
              <p className="py-4 text-center text-sm text-muted-foreground">Nothing is playing.</p>
            )}
          </PopoverContent>
        </Popover>
        {settings && (
          <>
            <VolumeControl
              icon={Volume2}
              label="Soundboard volume — what others hear"
              value={settings.masterVolume}
              onChange={(v) => void updateSettings({ masterVolume: v })}
            />
            <VolumeControl
              icon={Headphones}
              label={settings.monitorEnabled ? "Monitor volume — what you hear" : "Monitoring is off"}
              value={settings.monitorVolume}
              disabled={!settings.monitorEnabled}
              onChange={(v) => void updateSettings({ monitorVolume: v })}
            />
          </>
        )}
        <RoutingPill />
      </div>
    </footer>
  );
}
