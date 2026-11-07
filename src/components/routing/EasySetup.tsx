import { useState } from "react";
import { CheckCircle2, Download, Loader2, RotateCcw, Wand2, XCircle } from "lucide-react";

import { Button } from "@/components/ui/button";
import { api, errorMessage, type VerifyResult } from "@/lib/api";
import { useApp } from "@/store/app";

/** Set when the cable was installed but needs a restart; App finishes setup on the next launch. */
export const FINISH_SETUP_KEY = "blurt.finishSetupAfterRestart";

type Stage =
  | { kind: "idle" }
  | { kind: "busy"; text: string }
  | { kind: "done"; result: VerifyResult }
  | { kind: "needCable"; reason: string }
  | { kind: "restart" }
  | { kind: "error"; text: string };

/** Run automatic routing, then let the app's settings and route check catch up. */
export async function autoSetup() {
  const r = await api.autoSetupVirtualMic();
  useApp.getState().setRouteCheck(r);
  useApp.setState({ settings: await api.getSettings() });
  return r;
}

/** One button: find a working virtual mic, or install VB-CABLE and then configure it. */
export function EasySetup() {
  const windows = (useApp((s) => s.info?.platform) ?? "windows") === "windows";
  const [stage, setStage] = useState<Stage>({ kind: "idle" });

  const setup = async () => {
    setStage({ kind: "busy", text: "Looking for a virtual microphone and testing it…" });
    try {
      setStage({ kind: "done", result: await autoSetup() });
    } catch (e) {
      setStage(windows ? { kind: "needCable", reason: errorMessage(e) } : { kind: "error", text: errorMessage(e) });
    }
  };

  const install = async () => {
    setStage({ kind: "busy", text: "Downloading VB-CABLE… When its window opens, click “Install Driver”, then OK." });
    try {
      if (!(await api.installVirtualCable())) {
        localStorage.setItem(FINISH_SETUP_KEY, "1");
        setStage({ kind: "restart" });
        return;
      }
      setStage({ kind: "busy", text: "Installed. Testing it…" });
      setStage({ kind: "done", result: await autoSetup() });
    } catch (e) {
      setStage({ kind: "error", text: errorMessage(e) });
    }
  };

  return (
    <div className="space-y-3 rounded-xl border bg-card p-5">
      {stage.kind === "idle" && (
        <>
          <p className="text-sm text-muted-foreground">Blurt will find or install a virtual microphone, test it, and mix your real microphone in so people still hear you.</p>
          <Button size="lg" onClick={() => void setup()}>
            <Wand2 /> Set up my microphone
          </Button>
        </>
      )}
      {stage.kind === "busy" && (
        <p className="flex items-center gap-2 text-sm">
          <Loader2 className="size-4 animate-spin" /> {stage.text}
        </p>
      )}
      {stage.kind === "done" && (
        <div className="space-y-2 text-sm">
          <p className="flex items-center gap-2 font-medium text-live">
            <CheckCircle2 className="size-5" /> You're all set.
          </p>
          <p>
            Last step: in Discord, Zoom, Teams or your game, open voice settings and choose <strong>{stage.result.capture}</strong> as your microphone (input device).
          </p>
        </div>
      )}
      {stage.kind === "needCable" && (
        <div className="space-y-3 text-sm">
          <p>
            Your PC doesn't have a virtual microphone yet. Blurt can get <strong>VB-CABLE</strong>, a free driver by VB-Audio. It downloads from vb-audio.com, and Windows asks for
            permission once.
          </p>
          <div className="flex gap-2">
            <Button onClick={() => void install()}>
              <Download /> Install VB-CABLE
            </Button>
            <Button variant="ghost" onClick={() => void setup()}>
              <RotateCcw /> Try again
            </Button>
          </div>
          <p className="text-xs text-muted-foreground">{stage.reason}</p>
        </div>
      )}
      {stage.kind === "restart" && (
        <p className="text-sm">VB-CABLE is installed, but Windows needs a restart before it shows up. Restart your PC and Blurt will finish setting up on its own.</p>
      )}
      {stage.kind === "error" && (
        <div className="space-y-3 text-sm">
          <p className="flex items-start gap-2 text-destructive">
            <XCircle className="mt-0.5 size-4 shrink-0" /> {stage.text}
          </p>
          <Button variant="outline" onClick={() => void setup()}>
            <RotateCcw /> Try again
          </Button>
        </div>
      )}
    </div>
  );
}
