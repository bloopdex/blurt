import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AlertTriangle, CheckCircle2, ChevronDown, Circle, ExternalLink, Info, Loader2, RefreshCw, Volume2, Wand2, XCircle } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectGroup, SelectItem, SelectLabel, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { api, errorMessage, type Device, type VerifyResult } from "@/lib/api";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { currentCheck, isHardwareOutput, resolvedCapture, routingState, useApp } from "@/store/app";
import { routingCopy } from "./RoutingPill";

const DEFAULT = "__default__";
const NONE = "__none__";
const AUTO = "__auto__";

export function DriverGuide({ platform, defaultOpen = false }: { platform: string; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  const mac = platform === "macos";
  return (
    <div className="rounded-lg border">
      <button type="button" onClick={() => setOpen(!open)} aria-expanded={open} className="flex w-full items-center justify-between px-4 py-3 text-left text-sm font-medium">
        How to install a virtual microphone {mac ? "on macOS" : "on Windows"}
        <ChevronDown className={cn("size-4 transition-transform", open && "rotate-180")} />
      </button>
      {open && (
        <div className="space-y-3 border-t px-4 py-3 text-sm text-muted-foreground">
          <p>
            Apps can't create microphones by themselves, so Blurt plays into a free <strong className="text-foreground">virtual audio cable</strong>: a driver with a playback side
            (Blurt sends into it) and a recording side (Discord, Zoom, Teams, Meet and games listen to it).
          </p>
          {mac ? (
            <ol className="list-decimal space-y-1.5 pl-5">
              <li>
                Install <strong className="text-foreground">BlackHole 2ch</strong> (free, open source) from Existential Audio, or with Homebrew:{" "}
                <code className="rounded bg-surface-2 px-1">brew install blackhole-2ch</code>.
              </li>
              <li>Approve the installer's system prompts. If BlackHole doesn't appear, restart your Mac.</li>
              <li>
                Press <em>Rescan</em>, set <em>Blurt plays into</em> to <strong className="text-foreground">BlackHole 2ch</strong> and run the test.
              </li>
              <li>
                In Discord / Zoom, set the input device to <strong className="text-foreground">BlackHole 2ch</strong>.
              </li>
            </ol>
          ) : (
            <ol className="list-decimal space-y-1.5 pl-5">
              <li>
                Download <strong className="text-foreground">VB-CABLE</strong> (free donationware by VB-Audio) and unzip it.
              </li>
              <li>
                Right-click <code className="rounded bg-surface-2 px-1">VBCABLE_Setup_x64.exe</code> → <em>Run as administrator</em> → <em>Install Driver</em>. Restart if Windows
                asks.
              </li>
              <li>
                Press <em>Rescan</em>, set <em>Blurt plays into</em> to <strong className="text-foreground">CABLE Input (VB-Audio Virtual Cable)</strong> and run the test.
              </li>
              <li>
                In Discord → Voice & Video, set Input Device to <strong className="text-foreground">CABLE Output (VB-Audio Virtual Cable)</strong>.
              </li>
            </ol>
          )}
          <p>To keep talking normally, turn on “Mix in my microphone” below.</p>
          <Button variant="outline" size="sm" onClick={() => void openUrl(mac ? "https://existential.audio/blackhole/" : "https://vb-audio.com/Cable/")}>
            <ExternalLink /> {mac ? "Get BlackHole" : "Get VB-CABLE"}
          </Button>
        </div>
      )}
    </div>
  );
}

type StepState = "ok" | "warn" | "fail" | "todo";

function Step({ state, title, detail }: { state: StepState; title: string; detail?: string }) {
  const Icon = { ok: CheckCircle2, warn: AlertTriangle, fail: XCircle, todo: Circle }[state];
  return (
    <li className="flex gap-2.5 py-1.5">
      <Icon
        className={cn(
          "mt-0.5 size-4 shrink-0",
          state === "ok" && "text-live",
          state === "warn" && "text-warn",
          state === "fail" && "text-destructive",
          state === "todo" && "text-muted-foreground",
        )}
        aria-hidden="true"
      />
      <div className="min-w-0 text-sm">
        <span className="font-medium">{title}</span>
        {detail && <p className="text-xs text-muted-foreground">{detail}</p>}
      </div>
    </li>
  );
}

/** The four things that must all be true before people hear you — each shown separately. */
function RoutingChecklist({ check }: { check: VerifyResult | null }) {
  const { settings, engine, devices } = useApp();
  if (!settings) return null;
  const output = settings.virtualMicDevice;
  const capture = resolvedCapture(settings, devices);
  const outputs = devices?.outputs ?? [];
  const inputs = devices?.inputs ?? [];

  const playback: [StepState, string, string?] = !output
    ? ["todo", "Playback device", "Choose where Blurt sends your sounds."]
    : inputs.some((d) => d.name === output) && !outputs.some((d) => d.name === output)
      ? ["fail", "Incorrect device direction", `“${output}” is a recording device; Blurt needs a playback device.`]
      : isHardwareOutput(output, devices)
        ? ["fail", "These are real speakers or headphones", `Sound sent to “${output}” comes out into the room; it can't get into a microphone. Choose a virtual cable (or press Set up automatically). To hear sounds yourself, turn on “Hear sounds myself” below.`]
      : engine?.virtualMic.open
        ? ["ok", "Playback device connected", `${output} · ${engine.virtualMic.sampleRate} Hz`]
        : ["fail", "Playback device unavailable", engine?.virtualMic.error ?? `“${output}” isn't connected.`];

  const mic: [StepState, string, string?] = !capture
    ? ["todo", "Virtual microphone", "Choose the recording device other apps will use."]
    : outputs.some((d) => d.name === capture) && !inputs.some((d) => d.name === capture)
      ? ["fail", "Incorrect device direction", `“${capture}” is a playback device, not a microphone.`]
      : inputs.some((d) => d.name === capture)
        ? inputs.find((d) => d.name === capture)?.software === false
          ? ["fail", "This is a real microphone", `No app can put sound into “${capture}”. Other apps must use the virtual cable's microphone; use “Mix in my microphone” to send your voice through it.`]
          : ["ok", "Virtual microphone detected", capture]
        : ["fail", "Virtual microphone unavailable", `“${capture}” isn't connected.`];

  const tested = check ? `Tested ${relativeTime(check.checkedAt)}` : undefined;
  const signal: [StepState, string, string?] = !check
    ? ["todo", "Not verified yet", "Run the test to prove audio reaches the microphone."]
    : check.signalReceived
      ? ["ok", "Signal received", `${check.receivedDb.toFixed(0)} dBFS (sent ${check.expectedDb.toFixed(0)}) · ${tested}`]
      : ["fail", "Audio test failed — no signal", tested];
  const quality: [StepState, string, string?] = !check || !check.signalReceived
    ? ["todo", "Signal quality"]
    : check.qualityOk
      ? ["ok", "Signal quality good", `${(check.receivedDb - check.noiseDb).toFixed(0)} dB above noise`]
      : ["warn", "Signal quality poor", "Received, but too quiet or noisy for a call."];

  return (
    <ul className="rounded-lg bg-surface-2 px-3 py-1.5" aria-label="Routing status">
      {[playback, mic, signal, quality].map(([state, title, detail]) => (
        <Step key={title} state={state} title={title} detail={detail} />
      ))}
    </ul>
  );
}

export function VerifyButton() {
  const app = useApp();
  const setRouteCheck = app.setRouteCheck;
  const [running, setRunning] = useState<"test" | "fix" | "auto" | null>(null);
  const [result, setResult] = useState<VerifyResult | { error: string } | null>(null);
  const check = result && !("error" in result) ? result : currentCheck(app);
  // Received but well below what was sent (Fix level leaves 6 dB headroom, see routing.rs): the recording device's input level can usually make it up.
  const canFix = !!check && check.signalReceived && check.expectedDb - check.receivedDb > 9;
  const run = async (mode: "test" | "fix" | "auto" = "test") => {
    setRunning(mode);
    try {
      const r = mode === "auto" ? await api.autoSetupVirtualMic() : await api.verifyVirtualMic(mode === "fix");
      setRouteCheck(r);
      // Settings only: engine status arrives by event; a snapshot here could be mid-reopen and stale.
      if (mode === "auto") useApp.setState({ settings: await api.getSettings() });
      setResult(r);
    } catch (e) {
      setResult({ error: errorMessage(e) });
    } finally {
      setRunning(null);
    }
  };
  const tone = !result ? null : "error" in result ? "fail" : result.ok ? "ok" : result.signalReceived ? "warn" : "fail";
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap gap-2">
        <Button size="sm" onClick={() => void run("auto")} disabled={!!running}>
          {running === "auto" ? <Loader2 className="animate-spin" /> : <Wand2 />}
          {running === "auto" ? "Testing every virtual device…" : "Set up automatically"}
        </Button>
        <Button variant="outline" size="sm" onClick={() => void run()} disabled={!!running}>
          {running === "test" ? <Loader2 className="animate-spin" /> : <RefreshCw />}
          {running === "test" ? "Listening for the test tone…" : "Test virtual microphone"}
        </Button>
        {canFix && (
          <Button size="sm" variant="outline" onClick={() => void run("fix")} disabled={!!running}>
            {running === "fix" ? <Loader2 className="animate-spin" /> : <Volume2 />}
            {running === "fix" ? "Fixing level…" : `Fix level (+${(check.expectedDb - check.receivedDb - 6).toFixed(0)} dB)`}
          </Button>
        )}
      </div>
      {canFix && (
        <p className="text-xs text-muted-foreground">
          <b>Fix level</b> raises “{check.capture}”'s input level in Windows sound settings so your sounds arrive at a clear level (with 6 dB to spare, so loud sounds don't clip), then tests again.
        </p>
      )}
      <p className="text-xs text-muted-foreground">
        <b>Set up automatically</b> tests every virtual audio device on this computer, picks one that really reaches a microphone, fixes its level and mixes your own mic in. <b>Test</b> plays a 1-second beep into the playback device and listens for it on the virtual microphone — exactly what other apps hear. Anyone on a call will hear it.
      </p>
      {result && (
        <div
          role="status"
          className={cn(
            "flex gap-2 rounded-md border px-3 py-2 text-sm",
            tone === "ok" && "border-live/30 bg-live/10 text-live",
            tone === "warn" && "border-warn/40 bg-warn/10 text-warn",
            tone === "fail" && "border-destructive/40 bg-destructive/10 text-destructive",
          )}
        >
          {tone === "ok" ? <CheckCircle2 className="mt-0.5 size-4 shrink-0" /> : tone === "warn" ? <AlertTriangle className="mt-0.5 size-4 shrink-0" /> : <XCircle className="mt-0.5 size-4 shrink-0" />}
          <span>{"error" in result ? result.error : result.message}</span>
        </div>
      )}
    </div>
  );
}

function Row({ title, description, children }: { title: string; description?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-6 py-3">
      <div className="min-w-0">
        <div className="text-sm font-medium">{title}</div>
        {description && <div className="mt-0.5 text-xs text-muted-foreground">{description}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Level({ label, value, onChange, max = 150 }: { label: string; value: number; onChange: (v: number) => void; max?: number }) {
  return (
    <div className="flex items-center gap-3">
      <Slider aria-label={label} className="w-44" min={0} max={max} value={[Math.round(value * 100)]} onValueChange={([v]) => onChange(v / 100)} />
      <span className="w-10 text-right text-xs text-muted-foreground tabular-nums">{Math.round(value * 100)}%</span>
    </div>
  );
}

/** Label suffix for a playback device: what we know about it, strongest evidence first. */
function outputHint(d: Device, check: VerifyResult | undefined): string {
  if (check) return check.ok ? "verified" : check.signalReceived ? "verified · weak signal" : "tested · doesn't reach a mic";
  if (d.knownLoopback && d.pairedCapture) return d.knownLoopback;
  if (d.software && d.pairedCapture) return "software device · not verified";
  return d.software ? "software output" : "real device · others won't hear";
}

/** Device routing: playback side, recording side, passthrough, monitor, levels. */
export function AudioSetup({ compact = false }: { compact?: boolean }) {
  const app = useApp();
  const { settings, engine, devices, refreshDevices, updateSettings, info, routeChecks } = app;
  const [scanning, setScanning] = useState(false);
  useEffect(() => {
    void refreshDevices();
  }, [refreshDevices]);
  if (!settings) return null;

  const rescan = async () => {
    setScanning(true);
    await refreshDevices();
    setScanning(false);
  };
  const outputs = devices?.outputs ?? [];
  const inputs = devices?.inputs ?? [];
  const lastCheckFor = (output: string) => routeChecks.find((c) => c.output === output);
  const cables = outputs.filter((d) => d.knownLoopback && d.pairedCapture);
  const candidates = outputs.filter((d) => d.software && d.pairedCapture && !d.knownLoopback);
  const regular = outputs.filter((d) => !cables.includes(d) && !candidates.includes(d));
  const selected = outputs.find((d) => d.name === settings.virtualMicDevice);
  const capture = resolvedCapture(settings, devices);
  const check = currentCheck(app);
  const state = routingState(engine, settings, check, devices);
  // Name used only to show help for a popular app, never to classify the device.
  const nvidiaSelected = /nvidia broadcast/i.test(settings.virtualMicDevice ?? "") || /nvidia broadcast/i.test(settings.micDevice ?? "");

  return (
    <div className="space-y-6">
      <section className="space-y-4 rounded-xl border bg-card p-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="font-display text-base font-bold">Virtual microphone</h3>
            <p className="mt-0.5 text-sm text-muted-foreground">Blurt plays into a playback device; other apps listen to its recording side.</p>
          </div>
          <span className={cn("flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium", state === "ready" ? "bg-live/15 text-live" : "bg-warn/15 text-warn")}>
            {state === "ready" ? <CheckCircle2 className="size-3.5" /> : <AlertTriangle className="size-3.5" />}
            {routingCopy[state].label}
          </span>
        </div>

        {devices && cables.length === 0 && (
          <div className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2 text-sm text-warn">
            No recognised virtual cable found. Install one (instructions below) and press Rescan. Other software devices can be tried and verified with the test.
          </div>
        )}

        <div className="grid grid-cols-[1fr_1fr_auto] items-end gap-2">
          <div className="min-w-0 space-y-1.5">
            <Label>Blurt plays into</Label>
            <Select value={settings.virtualMicDevice ?? NONE} onValueChange={(v) => void updateSettings({ virtualMicDevice: v === NONE ? null : v, virtualMicCapture: null })}>
              <SelectTrigger className="w-full" aria-label="Playback device Blurt plays into">
                <SelectValue placeholder="Choose a playback device" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={NONE}>Not set</SelectItem>
                {(
                  [
                    ["Virtual cables", cables],
                    ["Software devices — test before use", candidates],
                    ["Speakers & other playback devices", regular],
                  ] as const
                ).map(
                  ([label, list]) =>
                    list.length > 0 && (
                      <SelectGroup key={label}>
                        <SelectLabel>{label}</SelectLabel>
                        {list.map((d) => {
                          const hint = outputHint(d, lastCheckFor(d.name));
                          return (
                            <SelectItem key={d.name} value={d.name}>
                              {d.name}
                              {hint && <span className="text-muted-foreground"> · {hint}</span>}
                            </SelectItem>
                          );
                        })}
                      </SelectGroup>
                    ),
                )}
                {settings.virtualMicDevice && !outputs.some((d) => d.name === settings.virtualMicDevice) && (
                  <SelectItem value={settings.virtualMicDevice}>
                    {settings.virtualMicDevice}
                    {devices ? " (not connected)" : ""}
                  </SelectItem>
                )}
              </SelectContent>
            </Select>
          </div>
          <div className="min-w-0 space-y-1.5">
            <Label>Other apps use this microphone</Label>
            <Select
              value={settings.virtualMicCapture ?? AUTO}
              disabled={!settings.virtualMicDevice}
              onValueChange={(v) => void updateSettings({ virtualMicCapture: v === AUTO ? null : v })}
            >
              <SelectTrigger className="w-full" aria-label="Recording device other apps use">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={AUTO}>{selected?.pairedCapture ? `Automatic · ${selected.pairedCapture}` : "Automatic (none found)"}</SelectItem>
                {(
                  [
                    ["Virtual & software microphones", inputs.filter((d) => d.software)],
                    ["Physical microphones", inputs.filter((d) => !d.software)],
                  ] as const
                ).map(
                  ([label, list]) =>
                    list.length > 0 && (
                      <SelectGroup key={label}>
                        <SelectLabel>{label}</SelectLabel>
                        {list.map((d) => (
                          <SelectItem key={d.name} value={d.name}>
                            {d.name}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    ),
                )}
                {settings.virtualMicCapture && !inputs.some((d) => d.name === settings.virtualMicCapture) && (
                  <SelectItem value={settings.virtualMicCapture}>{settings.virtualMicCapture} (not connected)</SelectItem>
                )}
              </SelectContent>
            </Select>
          </div>
          <Button variant="outline" onClick={() => void rescan()} disabled={scanning}>
            <RefreshCw className={cn(scanning && "animate-spin")} /> Rescan
          </Button>
        </div>

        <RoutingChecklist check={check} />

        {selected && !selected.knownLoopback && !check?.ok && (
          <p className="flex gap-2 rounded-md border px-3 py-2 text-xs text-muted-foreground">
            <Info className="mt-0.5 size-3.5 shrink-0" />
            {selected.software && selected.pairedCapture
              ? `“${selected.name}” is a software audio device with a recording side, but it isn't a known virtual cable. Some software devices process audio and send it to your speakers instead of their microphone — the test below shows whether this one works.`
              : `“${selected.name}” is a regular playback device. People only hear your sounds if another program routes it into a microphone — the test below shows whether that's the case.`}
          </p>
        )}
        {capture && check?.ok && (
          <p className="rounded-md bg-surface-2 px-3 py-2 text-sm">
            In Discord, Zoom or your game, choose <strong>{capture}</strong> as your microphone.
          </p>
        )}
        {settings.virtualMicDevice && <VerifyButton />}

        {nvidiaSelected && (
          <div className="space-y-1.5 rounded-md border border-primary/30 bg-primary/5 px-3 py-2.5 text-sm">
            <p className="font-medium">Using NVIDIA Broadcast with Blurt</p>
            <p className="text-muted-foreground">
              NVIDIA Broadcast's <em>speaker</em> device cleans up audio you <em>hear</em> and sends it to your headphones — it doesn't feed its microphone (Blurt's test confirms this
              on Windows). Its <em>microphone</em> carries your real voice after noise removal. To combine the two:
            </p>
            <ol className="list-decimal space-y-0.5 pl-5 text-muted-foreground">
              <li>Set “Blurt plays into” to a virtual cable (e.g. CABLE Input).</li>
              <li>Turn on “Mix in my microphone” and choose <strong className="text-foreground">Microphone (NVIDIA Broadcast)</strong>.</li>
              <li>In Discord, use the cable's recording side (e.g. CABLE Output) as your microphone.</li>
            </ol>
          </div>
        )}
        {!compact && devices && <DriverGuide platform={info?.platform ?? "windows"} defaultOpen={cables.length === 0} />}
      </section>

      <section className="rounded-xl border bg-card px-5 py-2">
        <Row title="Mix in my microphone" description="Sends your real voice through the virtual mic too, so you don't switch inputs in Discord.">
          <Switch checked={settings.micPassthrough} onCheckedChange={(v) => void updateSettings({ micPassthrough: v })} aria-label="Mix in my microphone" />
        </Row>
        {settings.micPassthrough && (
          <Row title="Lower my voice while a sound plays" description="Turns your microphone down to a quarter during sounds, so they come through clearly.">
            <Switch checked={settings.duckMic} onCheckedChange={(v) => void updateSettings({ duckMic: v })} aria-label="Lower my voice while a sound plays" />
          </Row>
        )}
        {settings.micPassthrough && (
          <Row
            title="Microphone"
            description={
              engine?.passthrough.error ??
              (engine?.passthrough.open ? `Live at ${engine.passthrough.sampleRate} Hz` : "Processed microphones such as NVIDIA Broadcast work here.")
            }
          >
            <Select value={settings.micDevice ?? DEFAULT} onValueChange={(v) => void updateSettings({ micDevice: v === DEFAULT ? null : v })}>
              <SelectTrigger className="w-64" aria-label="Microphone to mix in">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={DEFAULT}>System default microphone</SelectItem>
                {/* The virtual mic itself would feed back into itself. */}
                {inputs
                  .filter((d) => d.name !== capture)
                  .map((d) => (
                    <SelectItem key={d.name} value={d.name}>
                      {d.name}
                    </SelectItem>
                  ))}
              </SelectContent>
            </Select>
          </Row>
        )}
      </section>

      <section className="rounded-xl border bg-card px-5 py-2">
        <Row title="Soundboard volume" description="Your sounds, before mixing.">
          <Level label="Soundboard volume" value={settings.masterVolume} onChange={(v) => void updateSettings({ masterVolume: v })} />
        </Row>
        <Row title="Microphone volume" description="Your voice, when “Mix in my microphone” is on.">
          <Level label="Microphone volume" value={settings.micVolume} max={200} onChange={(v) => void updateSettings({ micVolume: v })} />
        </Row>
        <Row title="Combined output" description="Everything sent to the virtual microphone together.">
          <Level label="Combined output volume" value={settings.outputVolume} onChange={(v) => void updateSettings({ outputVolume: v })} />
        </Row>
      </section>

      <section className="rounded-xl border bg-card px-5 py-2">
        <Row title="Hear sounds myself" description="Also play sounds on your speakers or headphones (monitoring). Previews always use this device.">
          <Switch checked={settings.monitorEnabled} onCheckedChange={(v) => void updateSettings({ monitorEnabled: v })} aria-label="Hear sounds myself" />
        </Row>
        <Row title="Monitor device" description={engine?.monitor.error ?? (settings.monitorDevice ? undefined : "Follows your system default output.")}>
          <Select value={settings.monitorDevice ?? DEFAULT} onValueChange={(v) => void updateSettings({ monitorDevice: v === DEFAULT ? null : v })}>
            <SelectTrigger className="w-64" aria-label="Monitor device">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={DEFAULT}>System default output</SelectItem>
              {outputs
                .filter((d) => d.name !== settings.virtualMicDevice)
                .map((d) => (
                  <SelectItem key={d.name} value={d.name}>
                    {d.name}
                  </SelectItem>
                ))}
            </SelectContent>
          </Select>
        </Row>
        {!compact && (
          <Row title="Sample rate" description="Leave on device default unless your cable is set to a specific rate.">
            <Select value={String(settings.sampleRate ?? DEFAULT)} onValueChange={(v) => void updateSettings({ sampleRate: v === DEFAULT ? null : Number(v) })}>
              <SelectTrigger className="w-64">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={DEFAULT}>Device default{engine?.virtualMic.sampleRate ? ` (${engine.virtualMic.sampleRate} Hz)` : ""}</SelectItem>
                <SelectItem value="44100">44.1 kHz</SelectItem>
                <SelectItem value="48000">48 kHz</SelectItem>
              </SelectContent>
            </Select>
          </Row>
        )}
      </section>

      {!compact && (
        <section className="space-y-2 rounded-xl border bg-card p-5 text-sm text-muted-foreground">
          <h3 className="font-display text-base font-bold text-foreground">Troubleshooting</h3>
          <ul className="list-disc space-y-1.5 pl-5">
            <li>People hear nothing: in the other app, the microphone must be the cable's <em>recording</em> side{capture ? ` (${capture})` : ""}, not your headset.</li>
            <li>The test says “never reached”: that playback device doesn't loop into that microphone. Pick a real virtual cable.</li>
            <li>Sounds cut out or get muffled in Discord: turn off Noise Suppression and Echo Cancellation, and set Input Sensitivity manually.</li>
            <li>Very quiet: if the test reports a weak signal, raise the cable's level in your system's sound settings.</li>
            <li>A device was unplugged or reinstalled: Blurt notices within a few seconds, keeps your choice, and reconnects when it's back. It never switches to your speakers on its own.</li>
          </ul>
        </section>
      )}
    </div>
  );
}
