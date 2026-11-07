import { create } from "zustand";
import { toast } from "sonner";

import {
  api,
  errorMessage,
  type AppInfo,
  type DeviceList,
  type EngineStatus,
  type Settings,
  type Voice,
  type VerifyResult,
} from "@/lib/api";

export type View =
  | { kind: "all" }
  | { kind: "favorites" }
  | { kind: "recent" }
  | { kind: "downloaded" }
  | { kind: "category"; id: string }
  | { kind: "collection"; id: string }
  | { kind: "downloads" }
  | { kind: "settings"; tab?: SettingsTab };

export type SettingsTab = "general" | "audio" | "playback" | "shortcuts" | "stream" | "storage" | "about";

interface AppStore {
  view: View;
  settings: Settings | null;
  info: AppInfo | null;
  engine: EngineStatus | null;
  voices: Voice[];
  devices: DeviceList | null;
  routeChecks: VerifyResult[];
  setRouteCheck: (r: VerifyResult) => void;
  onboardingOpen: boolean;
  setView: (v: View) => void;
  loadSettings: () => Promise<void>;
  updateSettings: (patch: Partial<Settings>) => Promise<boolean>;
  refreshDevices: () => Promise<void>;
  setEngine: (s: EngineStatus) => void;
  setVoices: (v: Voice[]) => void;
  setOnboardingOpen: (open: boolean) => void;
}

export const useApp = create<AppStore>((set, get) => ({
  view: { kind: "all" },
  settings: null,
  info: null,
  engine: null,
  voices: [],
  devices: null,
  routeChecks: [],
  setRouteCheck: (r) =>
    set((st) => ({ routeChecks: [r, ...st.routeChecks.filter((c) => !(c.output === r.output && c.capture === r.capture))] })),
  onboardingOpen: false,

  setView: (view) => set({ view }),

  loadSettings: async () => {
    const [settings, info, engine, voices, routeChecks] = await Promise.all([
      api.getSettings(),
      api.appInfo(),
      api.getEngineStatus(),
      api.getVoices(),
      api.getRouteChecks(),
    ]);
    // An engine-status event that arrived while these were in flight is newer than the snapshot.
    set((st) => ({ settings, info, engine: st.engine ?? engine, voices, routeChecks, onboardingOpen: !settings.onboardingDone }));
  },

  /** Optimistic: the UI updates at once and rolls back if the backend rejects the change. */
  updateSettings: async (patch) => {
    const prev = get().settings;
    if (prev) set({ settings: { ...prev, ...patch } });
    try {
      const settings = await api.updateSettings(patch);
      set({ settings });
      return true;
    } catch (e) {
      set({ settings: prev });
      toast.error("Couldn't save that setting", { description: errorMessage(e) });
      return false;
    }
  },

  refreshDevices: async () => {
    try {
      set({ devices: await api.listDevices() });
    } catch (e) {
      toast.error("Couldn't list audio devices", { description: errorMessage(e) });
    }
  },

  setEngine: (engine) => set({ engine }),
  setVoices: (voices) => set({ voices }),
  setOnboardingOpen: (onboardingOpen) => set({ onboardingOpen }),
}));

/** One place that decides how the virtual-mic state is described everywhere in the UI. */
export type RoutingState = "ready" | "not-reaching" | "unavailable" | "not-configured" | "unknown";

/** The recording endpoint other apps use: explicit choice, else the playback device's pair. */
export function resolvedCapture(settings: Settings | null, devices: DeviceList | null): string | null {
  if (!settings?.virtualMicDevice) return null;
  return settings.virtualMicCapture ?? devices?.outputs.find((d) => d.name === settings.virtualMicDevice)?.pairedCapture ?? null;
}

/** A physical playback device (per OS metadata), as opposed to a virtual/software one. */
export function isHardwareOutput(name: string | null, devices: DeviceList | null | undefined): boolean {
  return devices?.outputs.find((d) => d.name === name)?.software === false;
}

/** Latest routing test for the configured pair, if one was run. */
export function currentCheck(s: Pick<AppStore, "settings" | "devices" | "routeChecks">): VerifyResult | null {
  const capture = resolvedCapture(s.settings, s.devices);
  return s.routeChecks.find((c) => c.output === s.settings?.virtualMicDevice && c.capture === capture) ?? null;
}

export function routingState(engine: EngineStatus | null, settings: Settings | null, check?: VerifyResult | null, devices?: DeviceList | null): RoutingState {
  if (!engine || !settings) return "unknown";
  if (!settings.virtualMicDevice) return "not-configured";
  if (!engine.virtualMic.open) return "unavailable";
  // Real speakers/headphones play into the room, never into a microphone.
  if (isHardwareOutput(settings.virtualMicDevice, devices) && !check?.signalReceived) return "not-reaching";
  // The device is open, but a test proved its audio doesn't come out of the microphone.
  return check && !check.signalReceived ? "not-reaching" : "ready";
}
