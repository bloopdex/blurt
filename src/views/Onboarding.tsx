import { useState } from "react";
import { ArrowRight, ChevronDown, Download, Keyboard, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Kbd } from "@/components/Kbd";
import { LogoMark, Wordmark } from "@/components/Logo";
import { AudioSetup, DriverGuide } from "@/components/routing/AudioSetup";
import { EasySetup } from "@/components/routing/EasySetup";
import { pickFilesToImport } from "@/lib/actions";
import { cn } from "@/lib/utils";
import { currentCheck, routingState, useApp } from "@/store/app";

const steps = ["Welcome", "Virtual mic", "Sounds", "Shortcuts"] as const;

/** First-launch guide. Every step can be skipped; nothing here blocks using the app. */
export function Onboarding() {
  const app = useApp();
  const { onboardingOpen, setOnboardingOpen, updateSettings, engine, settings, info, setView } = app;
  const [step, setStep] = useState(0);
  const [advanced, setAdvanced] = useState(false);
  const ready = routingState(engine, settings, currentCheck(app), app.devices) === "ready";

  const finish = () => {
    setOnboardingOpen(false);
    setStep(0);
    if (!settings?.onboardingDone) void updateSettings({ onboardingDone: true });
  };

  return (
    <Dialog open={onboardingOpen} onOpenChange={(o) => !o && finish()}>
      <DialogContent className="flex max-h-[88vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl" aria-describedby="onboarding-desc">
        <div className="flex items-center gap-2 border-b px-6 py-3">
          {steps.map((s, i) => (
            <button
              key={s}
              type="button"
              onClick={() => setStep(i)}
              className={cn(
                "flex items-center gap-2 rounded-full px-2.5 py-1 text-xs font-medium transition-colors",
                i === step ? "bg-primary/15 text-primary" : "text-muted-foreground hover:text-foreground",
              )}
              aria-current={i === step ? "step" : undefined}
            >
              <span className={cn("grid size-4 place-items-center rounded-full text-[10px]", i <= step ? "bg-primary text-primary-foreground" : "bg-surface-3")}>{i + 1}</span>
              {s}
            </button>
          ))}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-8 py-7">
          {step === 0 && (
            <div className="space-y-5 py-6 text-center">
              <LogoMark className="mx-auto size-16" />
              <DialogTitle className="font-display text-3xl font-extrabold tracking-tight">
                Welcome to <Wordmark className="text-3xl" />
              </DialogTitle>
              <DialogDescription id="onboarding-desc" className="mx-auto max-w-md text-base">
                Pick a sound, press a button, and everyone listening to your microphone hears it — on Discord, Zoom, Teams, Meet or in-game chat.
              </DialogDescription>
              <p className="mx-auto max-w-md text-sm text-muted-foreground">Setup takes one click. You can skip any step and come back from Settings.</p>
            </div>
          )}

          {step === 1 && (
            <div className="space-y-5">
              <div>
                <h2 className="font-display text-xl font-bold">Connect your microphone</h2>
                <p className="mt-1 text-sm text-muted-foreground">So that people on your call hear your sounds.</p>
              </div>
              <EasySetup />
              <button
                type="button"
                onClick={() => setAdvanced(!advanced)}
                aria-expanded={advanced}
                className="flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
              >
                Choose devices myself <ChevronDown className={cn("size-4 transition-transform", advanced && "rotate-180")} />
              </button>
              {advanced && (
                <>
                  <DriverGuide platform={info?.platform ?? "windows"} />
                  <AudioSetup compact />
                </>
              )}
            </div>
          )}

          {step === 2 && (
            <div className="space-y-5">
              <div>
                <h2 className="font-display text-xl font-bold">Add your first sounds</h2>
                <p className="mt-1 text-sm text-muted-foreground">Import files from your computer or grab a clip from a link. You can also drag files onto the window any time.</p>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <button
                  type="button"
                  onClick={() => {
                    finish();
                    void pickFilesToImport();
                  }}
                  className="rounded-xl border bg-card p-5 text-left transition-colors hover:border-primary/60"
                >
                  <Plus className="mb-3 size-6 text-primary" />
                  <p className="font-medium">Import files</p>
                  <p className="text-sm text-muted-foreground">MP3, WAV, OGG, FLAC, M4A…</p>
                </button>
                <button
                  type="button"
                  onClick={() => {
                    finish();
                    setView({ kind: "downloads" });
                  }}
                  className="rounded-xl border bg-card p-5 text-left transition-colors hover:border-primary/60"
                >
                  <Download className="mb-3 size-6 text-primary" />
                  <p className="font-medium">Download from a link</p>
                  <p className="text-sm text-muted-foreground">YouTube, TikTok, Instagram…</p>
                </button>
              </div>
            </div>
          )}

          {step === 3 && (
            <div className="space-y-5">
              <div>
                <h2 className="font-display text-xl font-bold">Fire sounds without switching windows</h2>
                <p className="mt-1 text-sm text-muted-foreground">
                  Give any sound a global shortcut: right-click it → <em>Edit…</em> → <em>Global shortcut</em>. Shortcuts work while you're in a game or call, even with Blurt minimised to the tray.
                </p>
              </div>
              <div className="space-y-2 rounded-xl border bg-card p-5 text-sm">
                <div className="flex items-center justify-between">
                  <span className="flex items-center gap-2">
                    <Keyboard className="size-4 text-muted-foreground" /> Example: play your favourite
                  </span>
                  <Kbd shortcut="control+alt+Digit1" />
                </div>
                <div className="flex items-center justify-between">
                  <span className="flex items-center gap-2">
                    <Keyboard className="size-4 text-muted-foreground" /> Stop all sounds
                  </span>
                  {settings?.stopAllShortcut ? <Kbd shortcut={settings.stopAllShortcut} /> : <span className="text-muted-foreground">Not set</span>}
                </div>
                <div className="flex items-center justify-between">
                  <span className="flex items-center gap-2">
                    <Keyboard className="size-4 text-muted-foreground" /> Replay the last sound
                  </span>
                  {settings?.replayLastShortcut ? <Kbd shortcut={settings.replayLastShortcut} /> : <span className="text-muted-foreground">Not set</span>}
                </div>
                <div className="flex items-center justify-between">
                  <span className="flex items-center gap-2">
                    <Keyboard className="size-4 text-muted-foreground" /> Open the overlay over your game
                  </span>
                  {settings?.overlayShortcut ? <Kbd shortcut={settings.overlayShortcut} /> : <span className="text-muted-foreground">Not set</span>}
                </div>
              </div>
              {!ready && (
                <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2 text-sm text-warn">
                  Your virtual microphone isn't connected yet, so only you will hear sounds for now. Finish it any time in Settings → Audio.
                </p>
              )}
            </div>
          )}
        </div>

        <div className="flex items-center justify-between border-t px-6 py-4">
          <Button variant="ghost" onClick={finish}>
            {step === steps.length - 1 ? "Close" : "Skip setup"}
          </Button>
          <div className="flex gap-2">
            {step > 0 && (
              <Button variant="outline" onClick={() => setStep(step - 1)}>
                Back
              </Button>
            )}
            {step < steps.length - 1 ? (
              <Button onClick={() => setStep(step + 1)}>
                {step === 0 ? "Get started" : "Next"} <ArrowRight />
              </Button>
            ) : (
              <Button onClick={finish}>Start blurting</Button>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
