import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

/* Schemas mirror the Rust structs (serde camelCase). Responses are parsed so a backend/frontend
   mismatch fails loudly at the boundary instead of as `undefined` deep in the UI. */

export const playbackModes = ["default", "restart", "toggle", "overlap", "hold"] as const;

export const soundSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  fileName: z.string(),
  originalName: z.string(),
  format: z.string(),
  durationMs: z.number(),
  fileSize: z.number(),
  sampleRate: z.number(),
  channels: z.number(),
  coverFile: z.string().nullable(),
  volume: z.number(),
  trimStartMs: z.number(),
  trimEndMs: z.number().nullable(),
  playbackMode: z.enum(playbackModes),
  categoryId: z.string().nullable(),
  collectionIds: z.array(z.string()),
  isFavorite: z.boolean(),
  isPinned: z.boolean(),
  playCount: z.number(),
  lastPlayedAt: z.number().nullable(),
  position: z.number(),
  shortcut: z.string().nullable(),
  waveform: z.array(z.number()),
  sourceUrl: z.string().nullable(),
  createdAt: z.number(),
  updatedAt: z.number(),
});
export type Sound = z.infer<typeof soundSchema>;

export const groupSchema = z.object({
  id: z.string(),
  kind: z.enum(["category", "collection"]),
  name: z.string(),
  icon: z.string(),
  color: z.string(),
  position: z.number(),
});
export type Group = z.infer<typeof groupSchema>;

const librarySchema = z.object({ sounds: z.array(soundSchema), groups: z.array(groupSchema) });

export const settingsSchema = z.object({
  theme: z.enum(["dark", "light", "system"]),
  startAtLogin: z.boolean(),
  launchMinimized: z.boolean(),
  closeToTray: z.boolean(),
  confirmDelete: z.boolean(),
  showNotifications: z.boolean(),
  showDockIcon: z.boolean(),
  onboardingDone: z.boolean(),
  autoUpdate: z.boolean(),
  virtualMicDevice: z.string().nullable(),
  virtualMicCapture: z.string().nullable(),
  monitorDevice: z.string().nullable(),
  monitorEnabled: z.boolean(),
  micDevice: z.string().nullable(),
  micPassthrough: z.boolean(),
  duckMic: z.boolean(),
  micVolume: z.number(),
  masterVolume: z.number(),
  outputVolume: z.number(),
  monitorVolume: z.number(),
  defaultSoundVolume: z.number(),
  sampleRate: z.number().nullable(),
  maxVoices: z.number(),
  playbackMode: z.enum(["restart", "toggle", "overlap"]),
  stopPrevious: z.boolean(),
  fadeInMs: z.number(),
  fadeOutMs: z.number(),
  normalize: z.boolean(),
  stopAllShortcut: z.string().nullable(),
  replayLastShortcut: z.string().nullable(),
  overlayShortcut: z.string().nullable(),
  randomSoundShortcut: z.string().nullable(),
  pttKey: z.string().nullable(),
  clipRecording: z.boolean(),
  clipSeconds: z.number(),
  clipShortcut: z.string().nullable(),
  remoteControl: z.boolean(),
  remoteToken: z.string(),
  browserExtension: z.boolean(),
  micMuted: z.boolean(),
  gridDensity: z.enum(["compact", "comfortable", "large"]),
  sort: z.enum(["manual", "name", "recent", "plays", "duration"]),
  sidebarCollapsed: z.boolean(),
});
export type Settings = z.infer<typeof settingsSchema>;

const sinkStatusSchema = z.object({
  device: z.string().nullable(),
  open: z.boolean(),
  sampleRate: z.number().nullable(),
  error: z.string().nullable(),
});
export const engineStatusSchema = z.object({
  virtualMic: sinkStatusSchema,
  monitor: sinkStatusSchema,
  passthrough: sinkStatusSchema,
});
export type EngineStatus = z.infer<typeof engineStatusSchema>;

export const voiceSchema = z.object({
  voiceId: z.number(),
  soundId: z.string().nullable(),
  name: z.string(),
  positionMs: z.number(),
  durationMs: z.number(),
  paused: z.boolean(),
  preview: z.boolean(),
  onMic: z.boolean(),
});
export type Voice = z.infer<typeof voiceSchema>;

const deviceSchema = z.object({
  name: z.string(),
  isDefault: z.boolean(),
  adapter: z.string().nullable(),
  software: z.boolean(),
  pairedCapture: z.string().nullable(),
  knownLoopback: z.string().nullable(),
});
export type Device = z.infer<typeof deviceSchema>;
const deviceListSchema = z.object({ outputs: z.array(deviceSchema), inputs: z.array(deviceSchema) });
export type DeviceList = z.infer<typeof deviceListSchema>;

export const verifySchema = z.object({
  output: z.string(),
  capture: z.string(),
  signalReceived: z.boolean(),
  qualityOk: z.boolean(),
  ok: z.boolean(),
  expectedDb: z.number(),
  receivedDb: z.number(),
  noiseDb: z.number(),
  message: z.string(),
  checkedAt: z.number(),
});
export type VerifyResult = z.infer<typeof verifySchema>;

const playOutcomeSchema = z.object({
  voiceId: z.number().nullable(),
  onMic: z.boolean(),
  warning: z.string().nullable(),
});
export type PlayOutcome = z.infer<typeof playOutcomeSchema>;

const importReportSchema = z.object({
  imported: z.array(soundSchema),
  failed: z.array(z.object({ path: z.string(), message: z.string() })),
});

export const mediaInfoSchema = z.object({
  url: z.string(),
  source: z.object({ id: z.string(), label: z.string() }),
  title: z.string(),
  thumbnail: z.string().nullable(),
  durationMs: z.number().nullable(),
  uploader: z.string().nullable(),
});
export type MediaInfo = z.infer<typeof mediaInfoSchema>;

export const downloadSchema = z.object({
  id: z.string(),
  url: z.string(),
  source: z.string(),
  title: z.string().nullable(),
  thumbnailUrl: z.string().nullable(),
  durationMs: z.number().nullable(),
  status: z.enum(["downloading", "ready", "imported", "failed", "cancelled"]),
  error: z.string().nullable(),
  tempFile: z.string().nullable(),
  soundId: z.string().nullable(),
  createdAt: z.number(),
});
export type DownloadRecord = z.infer<typeof downloadSchema>;

const toolStatusSchema = z.object({ installed: z.boolean(), managed: z.boolean(), version: z.string().nullable() });
export type ToolStatus = z.infer<typeof toolStatusSchema>;

const storageSchema = z.object({
  dataDir: z.string(),
  libraryDir: z.string(),
  coversDir: z.string(),
  soundsBytes: z.number(),
  coversBytes: z.number(),
  trashBytes: z.number(),
  tempBytes: z.number(),
  cacheBytes: z.number(),
  databaseBytes: z.number(),
});
export type StorageInfo = z.infer<typeof storageSchema>;

const shortcutProblemSchema = z.object({ soundId: z.string().nullable(), shortcut: z.string(), message: z.string() });
export type ShortcutProblem = z.infer<typeof shortcutProblemSchema>;

const appInfoSchema = z.object({ version: z.string(), platform: z.string(), coversDir: z.string() });
export type AppInfo = z.infer<typeof appInfoSchema>;

export type SoundPatch = Partial<
  Pick<Sound, "name" | "description" | "volume" | "trimStartMs" | "trimEndMs" | "playbackMode" | "categoryId" | "isFavorite" | "isPinned">
>;

/** Error thrown by every API call; `message` is already user-facing. */
export class ApiError extends Error {
  constructor(
    public kind: string,
    message: string,
  ) {
    super(message);
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>, schema?: z.ZodType<T>): Promise<T> {
  let raw: unknown;
  try {
    raw = await invoke(cmd, args);
  } catch (e) {
    if (e && typeof e === "object" && "message" in e) {
      const err = e as { kind?: string; message: string };
      throw new ApiError(err.kind ?? "unknown", err.message);
    }
    throw new ApiError("unknown", String(e));
  }
  if (!schema) return raw as T;
  const parsed = schema.safeParse(raw);
  if (!parsed.success) {
    console.error(`Unexpected response from ${cmd}`, parsed.error, raw);
    throw new ApiError("protocol", `Blurt received an unexpected response (${cmd}). Please restart the app.`);
  }
  return parsed.data;
}

export const api = {
  getLibrary: () => call("get_library", undefined, librarySchema),
  importFiles: (paths: string[], categoryId: string | null) =>
    call("import_files", { paths, categoryId }, importReportSchema),
  updateSound: (id: string, patch: SoundPatch) => call("update_sound", { id, patch }, soundSchema),
  deleteSounds: (ids: string[]) => call<void>("delete_sounds", { ids }),
  duplicateSound: (id: string) => call("duplicate_sound", { id }, soundSchema),
  setSoundCover: (id: string, path: string | null) => call("set_sound_cover", { id, path }, soundSchema),
  reorderSounds: (ids: string[]) => call<void>("reorder_sounds", { ids }),
  moveSounds: (ids: string[], categoryId: string | null) => call<void>("move_sounds", { ids, categoryId }),
  setCollectionMembership: (collectionId: string, ids: string[], member: boolean) =>
    call<void>("set_collection_membership", { collectionId, ids, member }),
  createGroup: (kind: Group["kind"], name: string, icon: string, color: string) =>
    call("create_group", { kind, name, icon, color }, groupSchema),
  updateGroup: (id: string, name: string, icon: string, color: string) =>
    call<void>("update_group", { id, name, icon, color }),
  deleteGroup: (id: string) => call<void>("delete_group", { id }),
  reorderGroups: (ids: string[]) => call<void>("reorder_groups", { ids }),

  playSound: (id: string) => call("play_sound", { id }, playOutcomeSchema),
  previewSound: (id: string, startMs: number, endMs: number | null, volume: number) =>
    call("preview_sound", { id, startMs, endMs, volume }, z.number().nullable()),
  previewFile: (path: string) => call("preview_file", { path }, z.number().nullable()),
  stopVoice: (voiceId: number) => call<void>("stop_voice", { voiceId }),
  stopAll: () => call<void>("stop_all"),
  replayLast: () => call("replay_last", undefined, playOutcomeSchema),
  hideOverlay: () => call<void>("hide_overlay"),
  overlayActivity: () => call<void>("overlay_activity"),
  saveClip: () => call("save_clip", undefined, soundSchema),
  textToSpeech: (text: string, voice: string | null) => call("text_to_speech", { text, voice }, soundSchema),
  ttsVoices: () => call("tts_voices", undefined, z.array(z.string())),
  setPaused: (voiceId: number | null, paused: boolean | null) => call<void>("set_paused", { voiceId, paused }),
  getVoices: () => call("get_voices", undefined, z.array(voiceSchema)),

  listDevices: () => call("list_devices", undefined, deviceListSchema),
  getEngineStatus: () => call("get_engine_status", undefined, engineStatusSchema),
  verifyVirtualMic: (fixLevel = false) => call("verify_virtual_mic", { fixLevel }, verifySchema),
  autoSetupVirtualMic: () => call("auto_setup_virtual_mic", undefined, verifySchema),
  installVirtualCable: () => call("install_virtual_cable", undefined, z.boolean()),
  getRouteChecks: () => call("get_route_checks", undefined, z.array(verifySchema)),

  getSettings: () => call("get_settings", undefined, settingsSchema),
  updateSettings: (patch: Partial<Settings>) => call("update_settings", { patch }, settingsSchema),
  setSoundShortcut: (id: string, shortcut: string | null) => call("set_sound_shortcut", { id, shortcut }, soundSchema),
  getShortcutProblems: () => call("get_shortcut_problems", undefined, z.array(shortcutProblemSchema)),

  ytdlpStatus: () => call("ytdlp_status", undefined, toolStatusSchema),
  installYtdlp: () => call("install_ytdlp", undefined, toolStatusSchema),
  probeUrl: (url: string) => call("probe_url", { url }, mediaInfoSchema),
  startDownload: (info: { url: string; title: string | null; thumbnail: string | null; durationMs: number | null }) =>
    call("start_download", { info }, downloadSchema),
  cancelDownload: (id: string) => call("cancel_download", { id }, z.boolean()),
  listDownloads: () => call("list_downloads", undefined, z.array(downloadSchema)),
  previewDownload: (id: string, startMs: number, endMs: number | null) =>
    call("preview_download", { id, startMs, endMs }, z.number().nullable()),
  importDownload: (args: {
    id: string;
    name: string;
    categoryId: string | null;
    trimStartMs: number;
    trimEndMs: number | null;
  }) => call("import_download", args, soundSchema),
  deleteDownload: (id: string) => call<void>("delete_download", { id }),

  storageInfo: () => call("storage_info", undefined, storageSchema),
  openLibraryFolder: () => call<void>("open_library_folder"),
  emptyTrash: () => call<void>("empty_trash"),
  clearCache: () => call<void>("clear_cache"),
  backupLibrary: (dest: string) => call<void>("backup_library", { dest }),
  restoreLibrary: (src: string) => call<void>("restore_library", { src }),
  appInfo: () => call("app_info", undefined, appInfoSchema),
};

export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
