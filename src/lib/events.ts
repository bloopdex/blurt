import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { toast } from "sonner";
import { z } from "zod";

import { engineStatusSchema, settingsSchema, soundSchema, voiceSchema, type EngineStatus } from "./api";
import { useApp } from "@/store/app";
import { useLibrary } from "@/store/library";
import { useDownloads } from "@/store/downloads";

const playedSchema = z.object({ id: z.string(), playCount: z.number(), lastPlayedAt: z.number().nullable() });
const progressSchema = z.object({ id: z.string(), percent: z.number() });
const problemsSchema = z.array(z.object({ soundId: z.string().nullable(), shortcut: z.string(), message: z.string() }));

/** Subscribe to backend events once at startup. Returns an unsubscribe function. */
export async function subscribe(onFileHover: (hovering: boolean) => void): Promise<() => void> {
  let lastStatus: EngineStatus | null = null;
  const unlisten = await Promise.all([
    listen("voices", (e) => {
      const v = z.array(voiceSchema).safeParse(e.payload);
      if (v.success) useApp.getState().setVoices(v.data);
    }),
    listen("engine-status", (e) => {
      const s = engineStatusSchema.safeParse(e.payload);
      if (!s.success) return;
      const prev = lastStatus;
      lastStatus = s.data;
      useApp.getState().setEngine(s.data);
      if (prev?.virtualMic.open && !s.data.virtualMic.open && s.data.virtualMic.device) {
        toast.error("Virtual microphone disconnected", {
          id: "mic-lost",
          description: `${s.data.virtualMic.device} stopped responding. Blurt will reconnect when it's back.`,
          duration: 10000,
        });
      } else if (prev && !prev.virtualMic.open && s.data.virtualMic.open && prev.virtualMic.error) {
        toast.success("Virtual microphone reconnected", { id: "mic-lost" });
      }
      if (prev?.monitor.open && !s.data.monitor.open) {
        toast.error("Output device disconnected", { id: "monitor-lost", description: s.data.monitor.error ?? undefined });
      }
    }),
    listen("devices-changed", () => {
      void useApp.getState().refreshDevices();
    }),
    listen("sound-added", (e) => {
      const s = soundSchema.safeParse(e.payload);
      if (!s.success) return;
      useLibrary.getState().upsert(s.data);
      toast.success(`Saved “${s.data.name}”`, { description: "It's in your library, ready to play." });
    }),
    listen("sound-played", (e) => {
      const p = playedSchema.safeParse(e.payload);
      if (p.success) useLibrary.getState().markPlayed(p.data.id, p.data.playCount, p.data.lastPlayedAt);
    }),
    listen("download-progress", (e) => {
      const p = progressSchema.safeParse(e.payload);
      if (p.success) useDownloads.getState().setProgress(p.data.id, p.data.percent);
    }),
    listen("download-started", () => {
      void useDownloads.getState().load();
      toast.info("Downloading from your browser", { description: "Trim it and add it from Downloads." });
    }),
    listen("settings-changed", (e) => {
      const s = settingsSchema.safeParse(e.payload);
      if (s.success) useApp.setState({ settings: s.data });
    }),
    listen("download-finished", (e) => {
      if (typeof e.payload === "string") void useDownloads.getState().finished(e.payload);
    }),
    listen("shortcut-problems", (e) => {
      const p = problemsSchema.safeParse(e.payload);
      if (!p.success) return;
      for (const problem of p.data) {
        toast.warning("A shortcut couldn't be registered", {
          id: `shortcut-${problem.shortcut}`,
          description: problem.message,
        });
      }
    }),
    listen("playback-error", (e) => {
      toast.error("Couldn't play that sound", { id: "play-error", description: String(e.payload) });
    }),
    getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") onFileHover(true);
      else if (p.type === "leave") onFileHover(false);
      else if (p.type === "drop") {
        onFileHover(false);
        if (p.paths.length) useLibrary.getState().setPendingImport(p.paths);
      }
    }),
  ]);
  return () => unlisten.forEach((u) => u());
}
