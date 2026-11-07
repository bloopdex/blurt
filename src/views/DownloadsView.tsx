import { useEffect, useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AlertCircle, Check, ExternalLink, Headphones, Link2, Loader2, Music2, RotateCcw, ShieldCheck, Square, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { z } from "zod";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, errorMessage, type DownloadRecord, type MediaInfo } from "@/lib/api";
import { formatClock, relativeTime } from "@/lib/format";
import { GroupIcon } from "@/lib/icons";
import { preview } from "@/lib/playback";
import { cn } from "@/lib/utils";
import { useApp } from "@/store/app";
import { useDownloads } from "@/store/downloads";
import { useLibrary } from "@/store/library";
import { useYtdlp } from "./SettingsView";

const urlSchema = z
  .string()
  .trim()
  .min(1, "Paste a link first.")
  .transform((s) => (/^[a-z]+:\/\//i.test(s) ? s : `https://${s}`))
  .pipe(z.url({ protocol: /^https?$/, error: "That doesn't look like a web link." }));

/** Client-side mirror of the backend's source detection, for an instant badge while typing. */
function detectSource(url: string): string | null {
  try {
    const host = new URL(/^[a-z]+:\/\//i.test(url) ? url : `https://${url}`).hostname.toLowerCase();
    const map: [string, string[]][] = [
      ["YouTube", ["youtube.com", "youtu.be"]],
      ["TikTok", ["tiktok.com"]],
      ["Instagram", ["instagram.com"]],
      ["X / Twitter", ["x.com", "twitter.com"]],
      ["SoundCloud", ["soundcloud.com"]],
      ["Reddit", ["reddit.com", "redd.it"]],
      ["Twitch", ["twitch.tv"]],
      ["Vimeo", ["vimeo.com"]],
    ];
    const hit = map.find(([, hosts]) => hosts.some((h) => host === h || host.endsWith(`.${h}`)));
    return hit ? hit[0] : host.includes(".") ? "Other site" : null;
  } catch {
    return null;
  }
}

export function DownloadsView() {
  const { items, load, add } = useDownloads();
  const tool = useYtdlp();
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [probing, setProbing] = useState(false);
  const [info, setInfo] = useState<MediaInfo | null>(null);
  const source = useMemo(() => (url.trim() ? detectSource(url.trim()) : null), [url]);

  useEffect(() => {
    void load();
  }, [load]);

  const probe = async () => {
    const parsed = urlSchema.safeParse(url);
    if (!parsed.success) return setError(parsed.error.issues[0]?.message ?? "Invalid link.");
    setError(null);
    setInfo(null);
    setProbing(true);
    try {
      setInfo(await api.probeUrl(parsed.data));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setProbing(false);
    }
  };

  const download = async () => {
    if (!info) return;
    try {
      add(await api.startDownload({ url: info.url, title: info.title, thumbnail: info.thumbnail, durationMs: info.durationMs }));
      setInfo(null);
      setUrl("");
    } catch (e) {
      toast.error("Couldn't start the download", { description: errorMessage(e) });
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="border-b px-6 py-4">
        <h1 className="font-display text-xl font-bold tracking-tight">Download sounds</h1>
        <p className="text-xs text-muted-foreground">Grab audio from YouTube, TikTok, Instagram and other sites.</p>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-3xl space-y-8 px-6 py-6">
          {tool.status && !tool.status.installed ? (
            <section className="space-y-3 rounded-xl border bg-card p-6">
              <h2 className="font-display text-lg font-bold">One-time setup</h2>
              <p className="text-sm text-muted-foreground">
                Blurt uses <strong className="text-foreground">yt-dlp</strong>, a free open-source downloader, to read links. It's about 18 MB and is fetched from its official GitHub release
                into Blurt's own folder.
              </p>
              <Button onClick={() => void tool.install()} disabled={tool.busy}>
                {tool.busy ? <Loader2 className="animate-spin" /> : null} {tool.busy ? "Installing…" : "Install downloader"}
              </Button>
            </section>
          ) : (
            <section className="space-y-4">
              <form
                className="flex gap-2"
                onSubmit={(e) => {
                  e.preventDefault();
                  void probe();
                }}
              >
                <div className="relative flex-1">
                  <Link2 className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
                  <Input
                    value={url}
                    onChange={(e) => {
                      setUrl(e.target.value);
                      setError(null);
                    }}
                    placeholder="Paste a link, e.g. https://youtu.be/…"
                    aria-label="Link to download"
                    aria-invalid={!!error}
                    className="h-11 pr-28 pl-9"
                  />
                  {source && <span className="absolute top-1/2 right-3 -translate-y-1/2 rounded-md bg-surface-3 px-2 py-0.5 text-xs text-muted-foreground">{source}</span>}
                </div>
                <Button type="submit" className="h-11 px-5" disabled={probing || !url.trim() || !tool.status}>
                  {probing ? <Loader2 className="animate-spin" /> : null} {probing ? "Checking…" : "Fetch"}
                </Button>
              </form>
              {error && (
                <p role="alert" className="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
                  <AlertCircle className="mt-0.5 size-4 shrink-0" /> {error}
                </p>
              )}
              {info && (
                <div className="flex gap-4 rounded-xl border bg-card p-4">
                  <div className="grid aspect-video w-48 shrink-0 place-items-center overflow-hidden rounded-lg bg-surface-2">
                    {info.thumbnail ? <img src={info.thumbnail} alt="" className="h-full w-full object-cover" /> : <Music2 className="size-6 text-muted-foreground" />}
                  </div>
                  <div className="flex min-w-0 flex-1 flex-col">
                    <span className="text-xs text-muted-foreground">{info.source.label}</span>
                    <p className="line-clamp-2 font-medium" data-selectable>
                      {info.title}
                    </p>
                    <p className="text-sm text-muted-foreground">
                      {[info.uploader, info.durationMs !== null ? formatClock(info.durationMs) : null].filter(Boolean).join(" · ")}
                    </p>
                    <div className="mt-auto flex gap-2 pt-3">
                      <Button onClick={() => void download()}>Download audio</Button>
                      <Button variant="ghost" onClick={() => setInfo(null)}>
                        Cancel
                      </Button>
                    </div>
                  </div>
                </div>
              )}
              <p className="flex items-start gap-2 text-xs text-muted-foreground">
                <ShieldCheck className="mt-0.5 size-3.5 shrink-0" />
                Only download content you own or have permission to use. Blurt doesn't sign in, use your cookies, or bypass DRM — private, age-restricted and protected content will be refused.
              </p>
            </section>
          )}

          <section className="space-y-3">
            <h2 className="text-[11px] font-semibold tracking-wider text-muted-foreground uppercase">History</h2>
            {items.length === 0 ? (
              <p className="rounded-xl border border-dashed p-8 text-center text-sm text-muted-foreground">Downloads you start appear here.</p>
            ) : (
              <ul className="space-y-2">
                {items.map((d) => (
                  <DownloadRow key={d.id} d={d} />
                ))}
              </ul>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}

function DownloadRow({ d }: { d: DownloadRecord }) {
  const { progress, remove, load, add } = useDownloads();
  const setView = useApp((s) => s.setView);
  const [reviewing, setReviewing] = useState(false);
  const pct = progress[d.id] ?? 0;

  const retry = async () => {
    try {
      add(await api.startDownload({ url: d.url, title: d.title, thumbnail: d.thumbnailUrl, durationMs: d.durationMs }));
      await api.deleteDownload(d.id);
      await load();
    } catch (e) {
      toast.error("Couldn't retry", { description: errorMessage(e) });
    }
  };

  return (
    <li className="rounded-xl border bg-card">
      <div className="flex items-center gap-3 p-3">
        <div className="grid aspect-video w-24 shrink-0 place-items-center overflow-hidden rounded-md bg-surface-2">
          {d.thumbnailUrl ? <img src={d.thumbnailUrl} alt="" className="h-full w-full object-cover" /> : <Music2 className="size-5 text-muted-foreground" />}
        </div>
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{d.title ?? d.url}</p>
          <p className="text-xs text-muted-foreground">
            {d.source} · {relativeTime(d.createdAt)}
            {d.durationMs !== null ? ` · ${formatClock(d.durationMs)}` : ""}
          </p>
          {d.status === "downloading" && <Progress value={pct} className="mt-2 h-1.5" aria-label={`Downloading, ${Math.round(pct)}%`} />}
          {d.status === "failed" && <p className="mt-1 text-xs text-destructive">{d.error}</p>}
          {d.status === "cancelled" && <p className="mt-1 text-xs text-muted-foreground">Cancelled</p>}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {d.status === "downloading" && (
            <>
              <span className="w-10 text-right text-xs text-muted-foreground tabular-nums">{Math.round(pct)}%</span>
              <Button variant="ghost" size="sm" onClick={() => void api.cancelDownload(d.id)}>
                Cancel
              </Button>
            </>
          )}
          {d.status === "ready" && !reviewing && <Button size="sm" onClick={() => setReviewing(true)}>Review & add</Button>}
          {d.status === "imported" && (
            <span className="flex items-center gap-1 text-xs text-live">
              <Check className="size-3.5" /> On your board
            </span>
          )}
          {(d.status === "failed" || d.status === "cancelled") && (
            <Button variant="ghost" size="sm" onClick={() => void retry()}>
              <RotateCcw /> Retry
            </Button>
          )}
          <Button variant="ghost" size="icon-sm" aria-label="Open original page" onClick={() => void openUrl(d.url)}>
            <ExternalLink />
          </Button>
          {d.status !== "downloading" && (
            <Button variant="ghost" size="icon-sm" aria-label="Remove from history" onClick={() => void remove(d.id)}>
              <Trash2 />
            </Button>
          )}
        </div>
      </div>
      {reviewing && d.status === "ready" && (
        <Review
          d={d}
          onDone={() => {
            setReviewing(false);
            void load();
            setView({ kind: "downloaded" });
          }}
          onCancel={() => setReviewing(false)}
        />
      )}
    </li>
  );
}

function parseTime(s: string): number | null {
  const t = s.trim();
  if (!t) return null;
  const m = /^(?:(\d+):)?(\d+(?:\.\d+)?)$/.exec(t);
  if (!m) return NaN;
  return Math.round(((m[1] ? Number(m[1]) * 60 : 0) + Number(m[2])) * 1000);
}

function Review({ d, onDone, onCancel }: { d: DownloadRecord; onDone: () => void; onCancel: () => void }) {
  const { groups, upsert } = useLibrary();
  const voices = useApp((s) => s.voices);
  const categories = groups.filter((g) => g.kind === "category").sort((a, b) => a.position - b.position);
  const [name, setName] = useState((d.title ?? "Downloaded sound").slice(0, 80));
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [busy, setBusy] = useState(false);
  const [voice, setVoice] = useState<number | null>(null);
  const playing = voice !== null && voices.some((v) => v.voiceId === voice);

  const startMs = parseTime(start) ?? 0;
  const endMs = parseTime(end);
  const trimError =
    Number.isNaN(startMs) || Number.isNaN(endMs ?? 0)
      ? "Use seconds (12.5) or minutes:seconds (1:05)."
      : endMs !== null && endMs - startMs < 50
        ? "End must be after start."
        : d.durationMs !== null && (startMs >= d.durationMs || (endMs ?? 0) > d.durationMs)
          ? `The clip is ${formatClock(d.durationMs)} long.`
          : null;
  const nameError = !name.trim() ? "Give it a name." : null;

  const togglePreview = async () => {
    if (playing && voice !== null) {
      await api.stopVoice(voice);
      return setVoice(null);
    }
    setVoice(await preview(() => api.previewDownload(d.id, startMs, endMs)));
  };

  const add = async () => {
    if (trimError || nameError) return;
    setBusy(true);
    try {
      if (voice !== null) await api.stopVoice(voice);
      const s = await api.importDownload({ id: d.id, name, categoryId, trimStartMs: startMs, trimEndMs: endMs });
      upsert(s);
      toast.success(`Added “${s.name}” to your board`);
      onDone();
    } catch (e) {
      toast.error("Couldn't add it", { description: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      className="space-y-4 border-t p-4"
      onSubmit={(e) => {
        e.preventDefault();
        void add();
      }}
    >
      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-1.5">
          <Label htmlFor={`name-${d.id}`}>Name</Label>
          <Input id={`name-${d.id}`} value={name} maxLength={80} onChange={(e) => setName(e.target.value)} aria-invalid={!!nameError} />
        </div>
        <div className="space-y-1.5">
          <Label>Category</Label>
          <Select value={categoryId ?? "none"} onValueChange={(v) => setCategoryId(v === "none" ? null : v)}>
            <SelectTrigger className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="none">No category</SelectItem>
              {categories.map((c) => (
                <SelectItem key={c.id} value={c.id}>
                  <GroupIcon name={c.icon} color={c.color} className="size-4" /> {c.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor={`start-${d.id}`}>Start at</Label>
          <Input id={`start-${d.id}`} value={start} placeholder="0:00" onChange={(e) => setStart(e.target.value)} aria-invalid={!!trimError} />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor={`end-${d.id}`}>End at</Label>
          <Input id={`end-${d.id}`} value={end} placeholder={d.durationMs !== null ? formatClock(d.durationMs) : "end"} onChange={(e) => setEnd(e.target.value)} aria-invalid={!!trimError} />
        </div>
      </div>
      {(trimError || nameError) && <p className="text-xs text-destructive">{nameError ?? trimError}</p>}
      <div className={cn("flex items-center justify-between")}>
        <Button type="button" variant="outline" size="sm" disabled={!!trimError} onClick={() => void togglePreview()}>
          {playing ? <Square className="fill-current" /> : <Headphones />} {playing ? "Stop" : "Preview selection"}
        </Button>
        <div className="flex gap-2">
          <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
            <X /> Close
          </Button>
          <Button type="submit" size="sm" disabled={busy || !!trimError || !!nameError}>
            {busy ? <Loader2 className="animate-spin" /> : <Check />} Add to board
          </Button>
        </div>
      </div>
      <p className="text-xs text-muted-foreground">You can fine-tune the trim on a waveform later in the sound's editor.</p>
    </form>
  );
}
