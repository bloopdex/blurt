import { describe, expect, it } from "vitest";

import type { DeviceList, EngineStatus, Settings, VerifyResult } from "@/lib/api";
import { currentCheck, resolvedCapture, routingState } from "./app";

const devices: DeviceList = {
  outputs: [
    { name: "Speakers (NVIDIA Broadcast)", isDefault: false, adapter: "ROOT\\UNNAMED_DEVICE\\0000", software: true, pairedCapture: "Microphone (NVIDIA Broadcast)", knownLoopback: null },
    { name: "CABLE Input (VB-Audio Virtual Cable)", isDefault: false, adapter: "ROOT\\VBAudioVACWDM\\0000", software: true, pairedCapture: "CABLE Output (VB-Audio Virtual Cable)", knownLoopback: "VB-CABLE" },
    { name: "Speakers (Realtek(R) Audio)", isDefault: true, adapter: "HDAUDIO\X", software: false, pairedCapture: null, knownLoopback: null },
  ],
  inputs: [],
};

const settings = (over: Partial<Settings>) => ({ virtualMicDevice: null, virtualMicCapture: null, ...over }) as Settings;
const engine = (open: boolean) => ({ virtualMic: { device: "x", open, sampleRate: 48000, error: null } }) as EngineStatus;
const check = (output: string, capture: string, signalReceived: boolean) => ({ output, capture, signalReceived }) as VerifyResult;

describe("routing state", () => {
  it("resolves the capture side: explicit choice wins over the driver pair", () => {
    expect(resolvedCapture(settings({ virtualMicDevice: "Speakers (NVIDIA Broadcast)" }), devices)).toBe("Microphone (NVIDIA Broadcast)");
    expect(resolvedCapture(settings({ virtualMicDevice: "Speakers (NVIDIA Broadcast)", virtualMicCapture: "Other" }), devices)).toBe("Other");
    expect(resolvedCapture(settings({}), devices)).toBeNull();
  });

  it("an open device is not 'ready' once a test proved it doesn't reach the mic", () => {
    const s = settings({ virtualMicDevice: "Speakers (NVIDIA Broadcast)" });
    const failed = check("Speakers (NVIDIA Broadcast)", "Microphone (NVIDIA Broadcast)", false);
    const c = currentCheck({ settings: s, devices, routeChecks: [failed] });
    expect(c).toBe(failed);
    expect(routingState(engine(true), s, c)).toBe("not-reaching");
    expect(routingState(engine(true), s, null)).toBe("ready");
    expect(routingState(engine(false), s, null)).toBe("unavailable");
    expect(routingState(engine(true), settings({}), null)).toBe("not-configured");
  });

  it("real speakers never count as reaching a microphone", () => {
    const s = settings({ virtualMicDevice: "Speakers (Realtek(R) Audio)" });
    expect(routingState(engine(true), s, null, devices)).toBe("not-reaching");
    expect(routingState(engine(true), settings({ virtualMicDevice: "CABLE Input (VB-Audio Virtual Cable)" }), null, devices)).toBe("ready");
  });

  it("only uses a check for the currently configured pair", () => {
    const s = settings({ virtualMicDevice: "CABLE Input (VB-Audio Virtual Cable)" });
    const other = check("Speakers (NVIDIA Broadcast)", "Microphone (NVIDIA Broadcast)", false);
    expect(currentCheck({ settings: s, devices, routeChecks: [other] })).toBeNull();
  });
});
