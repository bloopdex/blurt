import { useCallback, useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Archive, FolderOpen, RotateCcw, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { confirm } from "@/components/ConfirmDialog";
import { Kbd } from "@/components/Kbd";
import { LogoMark, Wordmark } from "@/components/Logo";
import { AudioSetup } from "@/components/routing/AudioSetup";
import { ShortcutRecorder } from "@/components/sounds/ShortcutRecorder";
import { api, errorMessage, type Settings, type ShortcutProblem, type StorageInfo } from "@/lib/api";
import { formatBytes } from "@/lib/format";
import { useApp, type SettingsTab } from "@/store/app";
import { useLibrary } from "@/store/library";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-1">
      <h3 className="px-1 pb-1 text-[11px] font-semibold tracking-wider text-muted-foreground uppercase">{title}</h3>
      <div className="divide-y rounded-xl border bg-card px-5">{children}</div>
    </section>
  );
}

function Row({ title, description, children }: { title: string; description?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 py-3.5">
      <div className="min-w-0">
        <div className="text-sm font-medium">{title}</div>
        {description && <div className="mt-0.5 text-xs text-muted-foreground">{description}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Toggle({ k, label }: { k: keyof Settings; label: string }) {
  const { settings, updateSettings } = useApp();
  return <Switch aria-label={label} checked={Boolean(settings?.[k])} onCheckedChange={(v) => void updateSettings({ [k]: v })} />;
}

function Percent({ k, label, max = 200 }: { k: "masterVolume" | "defaultSoundVolume"; label: string; max?: number }) {
  const { settings, updateSettings } = useApp();
  const v = Math.round((settings?.[k] ?? 1) * 100);
  return (
    <div className="flex items-center gap-3">
      <Slider aria-label={label} className="w-44" min={0} max={max} value={[v]} onValueChange={([x]) => void updateSettings({ [k]: x / 100 })} />
      <span className="w-10 text-right text-xs text-muted-foreground tabular-nums">{v}%</span>
    </div>
  );
}

function General() {
  const { settings, updateSettings, info, setOnboardingOpen } = useApp();
  if (!settings) return null;
  const mac = info?.platform === "macos";
  return (
    <div className="space-y-6">
      <Section title="Appearance">
        <Row title="Theme">
          <Select value={settings.theme} onValueChange={(v) => void updateSettings({ theme: v as Settings["theme"] })}>
            <SelectTrigger className="w-40">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="dark">Dark</SelectItem>
              <SelectItem value="light">Light</SelectItem>
              <SelectItem value="system">Match system</SelectItem>
            </SelectContent>
          </Select>
        </Row>
      </Section>
      <Section title="Startup & background">
        <Row title={mac ? "Open at login" : "Start with Windows"} description="Blurt starts in the background so your shortcuts are ready.">
          <Toggle k="startAtLogin" label="Start at login" />
        </Row>
        <Row title="Launch minimized" description="Start hidden in the tray instead of opening the window.">
          <Toggle k="launchMinimized" label="Launch minimized" />
        </Row>
        <Row title={mac ? "Keep running when the window is closed" : "Close to tray"} description="Closing the window keeps Blurt — and your shortcuts — running.">
          <Toggle k="closeToTray" label="Close to tray" />
        </Row>
        {mac && (
          <Row title="Show in Dock" description="When off, Blurt lives only in the menu bar while its window is closed.">
            <Toggle k="showDockIcon" label="Show in Dock" />
          </Row>
        )}
        <Row title="Notifications" description="Download results and device problems while Blurt is in the background.">
          <Toggle k="showNotifications" label="Notifications" />
        </Row>
      </Section>
      <Section title="Library">
        <Row title="Confirm before deleting sounds">
          <Toggle k="confirmDelete" label="Confirm before deleting" />
        </Row>
        <Row title="Setup guide" description="Walk through virtual microphone setup again.">
          <Button variant="outline" size="sm" onClick={() => setOnboardingOpen(true)}>
            Open guide
          </Button>
        </Row>
      </Section>
    </div>
  );
}

function Audio() {
  const { settings, updateSettings } = useApp();
  if (!settings) return null;
  return (
    <div className="space-y-6">
      <AudioSetup />
      <Section title="Library defaults">
        <Row title="Default volume for new sounds">
          <Percent k="defaultSoundVolume" label="Default sound volume" />
        </Row>
        <Row title="Sounds at once" description="Oldest sound stops when the limit is reached.">
          <Select value={String(settings.maxVoices)} onValueChange={(v) => void updateSettings({ maxVoices: Number(v) })}>
            <SelectTrigger className="w-28">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {[1, 2, 4, 8, 16, 32].map((n) => (
                <SelectItem key={n} value={String(n)}>
                  {n}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Row>
      </Section>
    </div>
  );
}

function Playback() {
  const { settings, updateSettings } = useApp();
  if (!settings) return null;
  const fades = [0, 30, 60, 150, 300, 600, 1000];
  return (
    <div className="space-y-6">
      <Section title="Behaviour">
        <Row title="Clicking a sound that's already playing" description="Sounds can override this in their editor.">
          <Select value={settings.playbackMode} onValueChange={(v) => void updateSettings({ playbackMode: v as Settings["playbackMode"] })}>
            <SelectTrigger className="w-56">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="restart">Restarts it</SelectItem>
              <SelectItem value="toggle">Stops it</SelectItem>
              <SelectItem value="overlap">Plays another copy on top</SelectItem>
            </SelectContent>
          </Select>
        </Row>
        <Row title="Stop other sounds when one starts" description="Only one sound at a time.">
          <Toggle k="stopPrevious" label="Stop other sounds" />
        </Row>
        <Row title="Even out loudness" description="Brings every sound to a similar peak level (non-destructive).">
          <Toggle k="normalize" label="Normalize" />
        </Row>
      </Section>
      <Section title="Fades">
        {(["fadeInMs", "fadeOutMs"] as const).map((k) => (
          <Row key={k} title={k === "fadeInMs" ? "Fade in" : "Fade out when stopped"}>
            <Select value={String(settings[k])} onValueChange={(v) => void updateSettings({ [k]: Number(v) })}>
              <SelectTrigger className="w-32">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {fades.map((ms) => (
                  <SelectItem key={ms} value={String(ms)}>
                    {ms === 0 ? "None" : `${ms} ms`}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Row>
        ))}
      </Section>
    </div>
  );
}

function Shortcuts() {
  const { settings, updateSettings } = useApp();
  const { sounds, setEditingId, upsert } = useLibrary();
  const [problems, setProblems] = useState<ShortcutProblem[]>([]);
  useEffect(() => {
    api.getShortcutProblems().then(setProblems, () => setProblems([]));
  }, [sounds, settings?.stopAllShortcut, settings?.replayLastShortcut, settings?.overlayShortcut, settings?.randomSoundShortcut]);
  if (!settings) return null;
  const bound = sounds.filter((s) => s.shortcut).sort((a, b) => a.name.localeCompare(b.name));

  const clear = async (id: string) => {
    try {
      upsert(await api.setSoundShortcut(id, null));
    } catch (e) {
      toast.error("Couldn't remove the shortcut", { description: errorMessage(e) });
    }
  };

  return (
    <div className="space-y-6">
      {problems.length > 0 && (
        <div className="space-y-1 rounded-xl border border-warn/40 bg-warn/10 p-4 text-sm text-warn">
          {problems.map((p) => (
            <p key={p.shortcut}>
              <Kbd shortcut={p.shortcut} /> {p.message}
            </p>
          ))}
        </div>
      )}
      <Section title="Global">
        <Row title="Stop all sounds" description="Works system-wide. Esc also stops everything while Blurt is focused.">
          <ShortcutRecorder label="Stop all shortcut" value={settings.stopAllShortcut} onChange={async (v) => void (await updateSettings({ stopAllShortcut: v }))} />
        </Row>
        <Row title="Replay last sound" description="Plays the last sound again from the start.">
          <ShortcutRecorder label="Replay last shortcut" value={settings.replayLastShortcut} onChange={async (v) => void (await updateSettings({ replayLastShortcut: v }))} />
        </Row>
        <Row title="Show overlay" description="Shows your pinned and top sounds on top of your game without leaving it; press 1–9 or 0 to play. Closes by itself after a few seconds, or press the shortcut again.">
          <ShortcutRecorder label="Overlay shortcut" value={settings.overlayShortcut} onChange={async (v) => void (await updateSettings({ overlayShortcut: v }))} />
        </Row>
        <Row title="Play a random sound" description="Never the same sound twice in a row.">
          <ShortcutRecorder label="Random sound shortcut" value={settings.randomSoundShortcut} onChange={async (v) => void (await updateSettings({ randomSoundShortcut: v }))} />
        </Row>
      </Section>
      <Section title="Sound shortcuts">
        {bound.length === 0 ? (
          <p className="py-4 text-sm text-muted-foreground">No sound has a shortcut yet. Right-click a sound → Edit to record one, e.g. Ctrl + Alt + 1.</p>
        ) : (
          bound.map((s) => (
            <Row key={s.id} title={s.name} description={s.playbackMode === "hold" ? "Hold to play" : undefined}>
              <div className="flex items-center gap-2">
                <Kbd shortcut={s.shortcut!} />
                <Button variant="ghost" size="xs" onClick={() => setEditingId(s.id)}>
                  Change
                </Button>
                <Button variant="ghost" size="xs" onClick={() => void clear(s.id)}>
                  Remove
                </Button>
              </div>
            </Row>
          ))
        )}
      </Section>
    </div>
  );
}

function Storage() {
  const [info, setInfo] = useState<StorageInfo | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const tool = useYtdlp();
  const refresh = useCallback(() => {
    api.storageInfo().then(setInfo, (e) => toast.error("Couldn't read storage usage", { description: errorMessage(e) }));
  }, []);
  useEffect(refresh, [refresh]);

  const run = async (label: string, fn: () => Promise<void>, success: string) => {
    setBusy(label);
    try {
      await fn();
      toast.success(success);
    } catch (e) {
      toast.error(`${label} failed`, { description: errorMessage(e) });
    } finally {
      setBusy(null);
      refresh();
    }
  };

  const backup = async () => {
    const dest = await save({
      title: "Save Blurt backup",
      defaultPath: `blurt-backup-${new Date().toISOString().slice(0, 10)}.zip`,
      filters: [{ name: "Blurt backup", extensions: ["zip"] }],
    });
    if (dest) await run("Backup", () => api.backupLibrary(dest), "Backup saved");
  };

  const restore = async () => {
    const src = await open({ title: "Restore a Blurt backup", filters: [{ name: "Blurt backup", extensions: ["zip"] }] });
    if (!src) return;
    const ok = await confirm({
      title: "Restore this backup?",
      description: "Your current library is replaced by the backup and Blurt restarts. The current data is kept as “before-restore” in the data folder.",
      confirmLabel: "Restore & restart",
      destructive: true,
    });
    if (ok) await run("Restore", () => api.restoreLibrary(src), "Restoring…");
  };

  return (
    <div className="space-y-6">
      <Section title="Usage">
        <Row title="Sounds" description={info?.libraryDir}>
          <div className="flex items-center gap-3">
            <span className="text-sm tabular-nums">{info ? formatBytes(info.soundsBytes + info.coversBytes) : "…"}</span>
            <Button variant="outline" size="sm" onClick={() => void api.openLibraryFolder().catch((e) => toast.error(errorMessage(e)))}>
              <FolderOpen /> Open folder
            </Button>
          </div>
        </Row>
        <Row title="Trash" description="Deleted sounds' audio files, kept until you empty it.">
          <div className="flex items-center gap-3">
            <span className="text-sm tabular-nums">{info ? formatBytes(info.trashBytes) : "…"}</span>
            <Button variant="outline" size="sm" disabled={!info?.trashBytes || !!busy} onClick={() => void run("Emptying trash", api.emptyTrash, "Trash emptied")}>
              <Trash2 /> Empty
            </Button>
          </div>
        </Row>
        <Row title="Cache" description="Decoded audio in memory and leftover temporary files.">
          <div className="flex items-center gap-3">
            <span className="text-sm tabular-nums">{info ? formatBytes(info.cacheBytes + info.tempBytes) : "…"}</span>
            <Button variant="outline" size="sm" disabled={!!busy} onClick={() => void run("Clearing cache", api.clearCache, "Cache cleared")}>
              Clear
            </Button>
          </div>
        </Row>
        <Row title="Database" description={info?.dataDir}>
          <span className="text-sm tabular-nums">{info ? formatBytes(info.databaseBytes) : "…"}</span>
        </Row>
      </Section>
      <Section title="Backup">
        <Row title="Back up library" description="Sounds, covers, categories, shortcuts and settings in one .zip file.">
          <Button variant="outline" size="sm" disabled={!!busy} onClick={() => void backup()}>
            <Archive /> {busy === "Backup" ? "Backing up…" : "Back up…"}
          </Button>
        </Row>
        <Row title="Restore from backup">
          <Button variant="outline" size="sm" disabled={!!busy} onClick={() => void restore()}>
            <RotateCcw /> Restore…
          </Button>
        </Row>
      </Section>
      <Section title="Downloader">
        <Row title="yt-dlp" description={tool.status ? (tool.status.installed ? `Version ${tool.status.version ?? "unknown"}${tool.status.managed ? " · managed by Blurt" : " · from your PATH"}` : "Not installed") : "Checking…"}>
          <Button variant="outline" size="sm" disabled={tool.busy || (tool.status?.installed && !tool.status.managed)} onClick={() => void tool.install()}>
            {tool.busy ? "Working…" : tool.status?.installed ? "Update" : "Install"}
          </Button>
        </Row>
      </Section>
    </div>
  );
}

export function useYtdlp() {
  const [status, setStatus] = useState<Awaited<ReturnType<typeof api.ytdlpStatus>> | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.ytdlpStatus().then(setStatus, () => setStatus(null));
  }, []);
  const install = async () => {
    setBusy(true);
    try {
      setStatus(await api.installYtdlp());
      toast.success("Downloader ready");
    } catch (e) {
      toast.error("Couldn't install the downloader", { description: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  };
  return { status, busy, install };
}

function About() {
  const info = useApp((s) => s.info);
  const link = (url: string, label: string) => (
    <Button variant="link" className="h-auto p-0" onClick={() => void openUrl(url)}>
      {label}
    </Button>
  );
  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4 rounded-xl border bg-card p-6">
        <LogoMark className="size-14" />
        <div>
          <Wordmark className="text-3xl" />
          <p className="text-sm text-muted-foreground">Say it with sound. Version {info?.version} · {info?.platform}</p>
        </div>
      </div>
      <Section title="Credits & licences">
        <Row title="Blurt" description="MIT License.">
          <span />
        </Row>
        <Row title="Built with" description="Tauri, React, Rust, cpal, Symphonia, SQLite, Radix UI, Lucide, Inter and Bricolage Grotesque.">
          <span />
        </Row>
        <Row title="yt-dlp" description="Downloaded on demand from its official releases (Unlicense). Blurt doesn't bypass DRM or sign in to sites — only download content you have the right to use.">
          {link("https://github.com/yt-dlp/yt-dlp", "Project page")}
        </Row>
        <Row title="Virtual audio drivers" description="VB-CABLE (VB-Audio, donationware) on Windows and BlackHole (GPL-3.0) on macOS are separate products installed by you.">
          <span />
        </Row>
      </Section>
      <Section title="Updates">
        <Row title="Check for updates" description="Automatic updates aren't enabled in this build; install new versions from the release page.">
          <span className="text-xs text-muted-foreground">Manual</span>
        </Row>
      </Section>
    </div>
  );
}

/** Mouse buttons 1/3/4 in DOM numbering are middle, back and forward. */
const MOUSE_KEYS: Record<number, string> = { 1: "Mouse3", 3: "Mouse4", 4: "Mouse5" };
const keyLabel = (k: string) => ({ Mouse3: "Middle mouse", Mouse4: "Mouse 4 (back)", Mouse5: "Mouse 5 (forward)" })[k] ?? k.replace(/^Key|^Digit/, "");

function PttKeyPicker() {
  const { settings, updateSettings } = useApp();
  const [listening, setListening] = useState(false);
  const pick = async (key: string) => {
    setListening(false);
    try {
      await updateSettings({ pttKey: key });
    } catch (e) {
      toast.error("Couldn't use that key", { description: errorMessage(e) });
    }
  };
  if (!settings) return null;
  return (
    <div className="flex items-center gap-2">
      <Button
        variant="outline"
        size="sm"
        className="min-w-40"
        onKeyDown={(e) => {
          if (!listening) return;
          e.preventDefault();
          if (e.key === "Escape") setListening(false);
          else void pick(e.code);
        }}
        onMouseDown={(e) => {
          if (listening && MOUSE_KEYS[e.button]) {
            e.preventDefault();
            void pick(MOUSE_KEYS[e.button]);
          }
        }}
        onClick={() => setListening(true)}
        onBlur={() => setListening(false)}
      >
        {listening ? "Press a key or mouse button…" : settings.pttKey ? keyLabel(settings.pttKey) : "Off — click to set"}
      </Button>
      {settings.pttKey && (
        <Button variant="ghost" size="xs" onClick={() => void updateSettings({ pttKey: null })}>
          Turn off
        </Button>
      )}
    </div>
  );
}

function Stream() {
  const { settings, updateSettings } = useApp();
  if (!settings) return null;
  const base = `http://127.0.0.1:7270`;
  const link = (path: string) => `${base}${path}${path.includes("?") ? "&" : "?"}token=${settings.remoteToken}`;
  const copy = (text: string) => void navigator.clipboard.writeText(text).then(() => toast.success("Copied"));
  return (
    <div className="space-y-6">
      <Section title="Push-to-talk">
        <Row
          title="Hold my push-to-talk key while sounds play"
          description="If Discord or your game uses push-to-talk, others only hear Blurt while that key is down. Blurt presses it for you during each sound. Side mouse buttons or F13–F24 work best. Some anti-cheat games ignore simulated keys."
        >
          <PttKeyPicker />
        </Row>
      </Section>

      <Section title="Instant clips">
        <Row title="Keep the last moments of what I hear" description="Records your speakers or headset in memory only (game, friends, music) so you can save a funny moment as a sound. Nothing is saved until you clip.">
          <Toggle k="clipRecording" label="Instant clips" />
        </Row>
        {settings.clipRecording && (
          <>
            <Row title="Clip length" description={`${settings.clipSeconds} seconds`}>
              <Slider aria-label="Clip length" className="w-44" min={5} max={120} step={5} value={[settings.clipSeconds]} onValueChange={([v]) => void updateSettings({ clipSeconds: v })} />
            </Row>
            <Row title="Save a clip" description="Works in-game. The clip appears in your library as “Clip #…”.">
              <div className="flex items-center gap-2">
                <ShortcutRecorder label="Clip shortcut" value={settings.clipShortcut} onChange={async (v) => void (await updateSettings({ clipShortcut: v }))} />
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void api.saveClip().catch((e) => toast.error("Couldn't save the clip", { description: errorMessage(e) }))}
                >
                  Clip now
                </Button>
              </div>
            </Row>
          </>
        )}
      </Section>

      <Section title="Stream Deck & remote control">
        <Row title="Allow Stream Deck, Touch Portal and Streamer.bot" description="Turns on a private link on this PC only (port 7270). Each link includes a secret key.">
          <Toggle k="remoteControl" label="Remote control" />
        </Row>
        {settings.remoteControl && settings.remoteToken && (
          <div className="space-y-2 py-3.5 text-xs">
            <p className="text-muted-foreground">
              In Stream Deck use <b>System → Website</b> with “GET request in background”. Replace <code>NAME</code> with a sound's name.
            </p>
            {[
              ["Play a sound", "/play?name=NAME"],
              ["Random sound", "/random"],
              ["Replay last", "/replay"],
              ["Stop all", "/stop"],
              ["Save a clip", "/clip"],
              ["Toggle overlay", "/overlay"],
            ].map(([label, path]) => (
              <div key={path} className="flex items-center gap-2">
                <span className="w-28 shrink-0 text-muted-foreground">{label}</span>
                <code className="min-w-0 flex-1 truncate rounded bg-surface-2 px-2 py-1" data-selectable>
                  {link(path)}
                </code>
                <Button variant="ghost" size="xs" onClick={() => copy(link(path))}>
                  Copy
                </Button>
              </div>
            ))}
          </div>
        )}
      </Section>
    </div>
  );
}

const tabs: { id: SettingsTab; label: string; body: () => React.ReactNode }[] = [
  { id: "general", label: "General", body: General },
  { id: "audio", label: "Audio", body: Audio },
  { id: "playback", label: "Playback", body: Playback },
  { id: "shortcuts", label: "Shortcuts", body: Shortcuts },
  { id: "stream", label: "Stream & game", body: Stream },
  { id: "storage", label: "Storage", body: Storage },
  { id: "about", label: "About", body: About },
];

export function SettingsView() {
  const { view, setView } = useApp();
  const tab = view.kind === "settings" ? (view.tab ?? "general") : "general";
  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="border-b px-6 py-4">
        <h1 className="font-display text-xl font-bold tracking-tight">Settings</h1>
      </header>
      <Tabs value={tab} onValueChange={(t) => setView({ kind: "settings", tab: t as SettingsTab })} className="min-h-0 flex-1 gap-0">
        <div className="border-b px-6 py-2">
          <TabsList>
            {tabs.map((t) => (
              <TabsTrigger key={t.id} value={t.id} className="px-3">
                {t.label}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto">
          {tabs.map((t) => (
            <TabsContent key={t.id} value={t.id} className="mx-auto max-w-3xl px-6 py-6">
              <t.body />
            </TabsContent>
          ))}
        </div>
      </Tabs>
    </div>
  );
}
