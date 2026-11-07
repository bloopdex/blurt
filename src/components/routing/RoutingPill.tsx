import { Mic, MicOff } from "lucide-react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { currentCheck, routingState, useApp, type RoutingState } from "@/store/app";

export const routingCopy: Record<RoutingState, { label: string; detail: string }> = {
  ready: { label: "Mic live", detail: "Sounds are going into your virtual microphone." },
  "not-reaching": { label: "Mic not reached", detail: "This device doesn't feed the microphone other apps listen to. Only you will hear sounds." },
  unavailable: { label: "Mic offline", detail: "The virtual microphone device isn't available. Only you will hear sounds." },
  "not-configured": { label: "Set up mic", detail: "No virtual microphone is set up yet. Only you will hear sounds." },
  unknown: { label: "Starting…", detail: "Checking audio devices." },
};

/** The always-visible answer to "will people on my call hear this?" */
export function RoutingPill({ compact = false }: { compact?: boolean }) {
  const app = useApp();
  const { engine, settings, setView } = app;
  const state = routingState(engine, settings, currentCheck(app), app.devices);
  const copy = routingCopy[state];
  const ready = state === "ready";
  const device = settings?.virtualMicDevice;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={() => setView({ kind: "settings", tab: "audio" })}
          aria-label={`${copy.label}. ${copy.detail} Open audio setup.`}
          className={cn(
            "flex items-center gap-2 rounded-full border text-xs font-medium transition-colors outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring",
            compact ? "size-8 justify-center" : "h-7 px-2.5",
            ready ? "border-live/30 text-live" : state === "unknown" ? "text-muted-foreground" : "border-warn/40 text-warn",
          )}
        >
          <span className="relative flex size-2 shrink-0">
            {ready && <span className="absolute inline-flex size-full animate-ping rounded-full bg-live opacity-50 motion-reduce:hidden" />}
            <span className={cn("relative inline-flex size-2 rounded-full", ready ? "bg-live" : "bg-current")} />
          </span>
          {!compact && (
            <>
              {ready ? <Mic className="size-3.5" /> : <MicOff className="size-3.5" />}
              <span>{copy.label}</span>
            </>
          )}
        </button>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-64">
        <p>{copy.detail}</p>
        {device && <p className="mt-1 opacity-70">{device}</p>}
      </TooltipContent>
    </Tooltip>
  );
}
