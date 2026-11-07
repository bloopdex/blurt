import { describe, expect, it } from "vitest";

import type { Sound } from "./api";
import { canReorder, effectiveDuration, visibleSounds } from "./filter";
import { eventToAccelerator, formatDuration, shortcutParts } from "./format";

function sound(over: Partial<Sound>): Sound {
  return {
    id: "x",
    name: "x",
    description: "",
    fileName: "x.wav",
    originalName: "x.wav",
    format: "wav",
    durationMs: 1000,
    fileSize: 1,
    sampleRate: 48000,
    channels: 2,
    coverFile: null,
    volume: 1,
    trimStartMs: 0,
    trimEndMs: null,
    playbackMode: "default",
    categoryId: null,
    collectionIds: [],
    isFavorite: false,
    isPinned: false,
    playCount: 0,
    lastPlayedAt: null,
    position: 0,
    shortcut: null,
    waveform: [],
    sourceUrl: null,
    createdAt: 0,
    updatedAt: 0,
    ...over,
  };
}

describe("visibleSounds", () => {
  const sounds = [
    sound({ id: "a", name: "Air horn", position: 2, isFavorite: true, categoryId: "memes" }),
    sound({ id: "b", name: "Bruh", position: 0, collectionIds: ["stream"], lastPlayedAt: 5 }),
    sound({ id: "c", name: "Crowd cheer", position: 1, isPinned: true, lastPlayedAt: 9, sourceUrl: "https://x" }),
  ];

  it("filters by view", () => {
    expect(visibleSounds(sounds, { kind: "favorites" }, "", "manual").map((s) => s.id)).toEqual(["a"]);
    expect(visibleSounds(sounds, { kind: "category", id: "memes" }, "", "manual").map((s) => s.id)).toEqual(["a"]);
    expect(visibleSounds(sounds, { kind: "collection", id: "stream" }, "", "manual").map((s) => s.id)).toEqual(["b"]);
    expect(visibleSounds(sounds, { kind: "downloaded" }, "", "manual").map((s) => s.id)).toEqual(["c"]);
  });

  it("orders recent by last play and pins first elsewhere", () => {
    expect(visibleSounds(sounds, { kind: "recent" }, "", "manual").map((s) => s.id)).toEqual(["c", "b"]);
    expect(visibleSounds(sounds, { kind: "all" }, "", "manual").map((s) => s.id)).toEqual(["c", "b", "a"]);
    expect(visibleSounds(sounds, { kind: "all" }, "", "name").map((s) => s.id)).toEqual(["c", "a", "b"]);
  });

  it("matches every search term", () => {
    expect(visibleSounds(sounds, { kind: "all" }, "air horn", "manual").map((s) => s.id)).toEqual(["a"]);
    expect(visibleSounds(sounds, { kind: "all" }, "horn cheer", "manual")).toEqual([]);
  });

  it("only allows manual reordering of the stored order", () => {
    expect(canReorder({ kind: "all" }, "", "manual")).toBe(true);
    expect(canReorder({ kind: "all" }, "x", "manual")).toBe(false);
    expect(canReorder({ kind: "all" }, "", "name")).toBe(false);
    expect(effectiveDuration(sound({ trimStartMs: 200, trimEndMs: 700 }))).toBe(500);
  });
});

describe("format", () => {
  it("formats durations and shortcuts", () => {
    expect(formatDuration(1500)).toBe("1.5s");
    expect(formatDuration(75_000)).toBe("1:15");
    // Labels follow the OS the test runs on (⌥ on macOS).
    expect(shortcutParts("control+alt+Digit1").slice(1)).toEqual([/Mac/i.test(navigator.platform) ? "⌥" : "Alt", "1"]);
    const base = { ctrlKey: true, altKey: true, shiftKey: false, metaKey: false };
    expect(eventToAccelerator({ ...base, key: "1", code: "Digit1" })).toBe("Control+Alt+Digit1");
    expect(eventToAccelerator({ ...base, key: "Control", code: "ControlLeft" })).toBeNull();
  });
});
