import { useEffect, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Headphones, ImagePlus, Square, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Textarea } from "@/components/ui/textarea";
import { api, errorMessage, type Sound } from "@/lib/api";
import { clearCover, deleteSounds, pickCover } from "@/lib/actions";
import { formatBytes, formatDuration, relativeTime } from "@/lib/format";
import { GroupIcon } from "@/lib/icons";
import { preview } from "@/lib/playback";
import { useApp } from "@/store/app";
import { useLibrary } from "@/store/library";
import { ShortcutRecorder } from "./ShortcutRecorder";
import { TrimWaveform } from "./Waveform";

const modeLabels: Record<Sound["playbackMode"], string> = {
  default: "Use default",
  restart: "Restart when clicked again",
  toggle: "Stop when clicked again",
  overlap: "Layer on top (overlap)",
  hold: "Hold shortcut to play",
};

export function SoundEditor() {
  const { editingId, setEditingId, sounds } = useLibrary();
  const sound = sounds.find((s) => s.id === editingId);
  return (
    <Dialog open={!!sound} onOpenChange={(o) => !o && setEditingId(null)}>
      {sound && <EditorBody key={sound.id} sound={sound} onClose={() => setEditingId(null)} />}
    </Dialog>
  );
}

function EditorBody({ sound, onClose }: { sound: Sound; onClose: () => void }) {
  const { groups, update, upsert } = useLibrary();
  const { info, voices } = useApp();
  const [name, setName] = useState(sound.name);
  const [description, setDescription] = useState(sound.description);
  const [volume, setVolume] = useState(sound.volume);
  const [mode, setMode] = useState(sound.playbackMode);
  const [categoryId, setCategoryId] = useState(sound.categoryId);
  const [trim, setTrim] = useState({ start: sound.trimStartMs, end: sound.trimEndMs ?? sound.durationMs });
  const [saving, setSaving] = useState(false);
  const [previewVoice, setPreviewVoice] = useState<number | null>(null);
  const categories = useMemo(() => groups.filter((g) => g.kind === "category").sort((a, b) => a.position - b.position), [groups]);
  const previewing = previewVoice !== null && voices.some((v) => v.voiceId === previewVoice);

  useEffect(() => () => void (previewVoice !== null && api.stopVoice(previewVoice)), [previewVoice]);

  const nameError = name.trim().length === 0 ? "Give the sound a name." : name.trim().length > 80 ? "Up to 80 characters." : null;

  const togglePreview = async () => {
    if (previewing && previewVoice !== null) {
      await api.stopVoice(previewVoice);
      setPreviewVoice(null);
      return;
    }
    const end = trim.end >= sound.durationMs ? null : trim.end;
    setPreviewVoice(await preview(() => api.previewSound(sound.id, trim.start, end, volume)));
  };

  const save = async () => {
    if (nameError) return;
    setSaving(true);
    const saved = await update(sound.id, {
      name,
      description,
      volume,
      playbackMode: mode,
      categoryId,
      trimStartMs: trim.start,
      trimEndMs: trim.end >= sound.durationMs ? null : trim.end,
    });
    setSaving(false);
    if (saved) {
      toast.success("Saved");
      onClose();
    }
  };

  const setShortcut = async (acc: string | null) => {
    try {
      upsert(await api.setSoundShortcut(sound.id, acc));
    } catch (e) {
      toast.error("Couldn't set that shortcut", { description: errorMessage(e) });
    }
  };

  const cover = sound.coverFile && info ? convertFileSrc(`${info.coversDir}/${sound.coverFile}`) : null;

  return (
    <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle>Edit sound</DialogTitle>
        <DialogDescription>Changes to the trim, volume and behaviour apply the next time it plays.</DialogDescription>
      </DialogHeader>
      <form
        className="space-y-6"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <div className="flex gap-4">
          <div className="w-36 shrink-0 space-y-2">
            <div className="grid aspect-square place-items-center overflow-hidden rounded-xl bg-surface-2">
              {cover ? <img src={cover} alt="Cover" className="h-full w-full object-cover" /> : <ImagePlus className="size-6 text-muted-foreground" />}
            </div>
            <div className="flex gap-1">
              <Button type="button" variant="outline" size="xs" className="flex-1" onClick={() => void pickCover(sound.id)}>
                {cover ? "Change" : "Add cover"}
              </Button>
              {cover && (
                <Button type="button" variant="ghost" size="icon-xs" aria-label="Remove cover" onClick={() => void clearCover(sound.id)}>
                  <Trash2 />
                </Button>
              )}
            </div>
          </div>
          <div className="min-w-0 flex-1 space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="sound-name">Name</Label>
              <Input id="sound-name" value={name} maxLength={80} aria-invalid={!!nameError} onChange={(e) => setName(e.target.value)} />
              {nameError && <p className="text-xs text-destructive">{nameError}</p>}
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="sound-desc">Description</Label>
              <Textarea id="sound-desc" rows={2} maxLength={500} value={description} placeholder="Optional notes" onChange={(e) => setDescription(e.target.value)} />
            </div>
          </div>
        </div>

        <section className="space-y-2">
          <div className="flex items-center justify-between">
            <Label>Trim</Label>
            <div className="flex items-center gap-3 text-xs text-muted-foreground tabular-nums">
              <span>
                {formatDuration(trim.start)} – {formatDuration(trim.end)} · {formatDuration(trim.end - trim.start)}
              </span>
              <Button type="button" variant="outline" size="xs" onClick={() => void togglePreview()}>
                {previewing ? <Square className="fill-current" /> : <Headphones />} {previewing ? "Stop" : "Preview"}
              </Button>
              {(trim.start > 0 || trim.end < sound.durationMs) && (
                <Button type="button" variant="ghost" size="xs" onClick={() => setTrim({ start: 0, end: sound.durationMs })}>
                  Reset
                </Button>
              )}
            </div>
          </div>
          <TrimWaveform
            peaks={sound.waveform}
            durationMs={sound.durationMs}
            startMs={trim.start}
            endMs={trim.end}
            onChange={(start, end) => setTrim({ start, end })}
          />
          <p className="text-xs text-muted-foreground">Trimming is non-destructive; the original file is kept. Previews play only on your speakers.</p>
        </section>

        <div className="grid grid-cols-2 gap-x-6 gap-y-5">
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
            <Label>When clicked again</Label>
            <Select value={mode} onValueChange={(v) => setMode(v as Sound["playbackMode"])}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {Object.entries(modeLabels).map(([k, label]) => (
                  <SelectItem key={k} value={k}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2.5">
            <div className="flex justify-between">
              <Label>Volume</Label>
              <span className="text-xs text-muted-foreground tabular-nums">{Math.round(volume * 100)}%</span>
            </div>
            <Slider aria-label="Sound volume" min={0} max={200} step={1} value={[Math.round(volume * 100)]} onValueChange={([v]) => setVolume(v / 100)} />
          </div>
          <div className="space-y-1.5">
            <Label>Global shortcut</Label>
            <ShortcutRecorder label="Shortcut" value={sound.shortcut} onChange={setShortcut} />
            <p className="text-xs text-muted-foreground">Works even when Blurt is minimised.{mode === "hold" ? " Hold the keys to play; release to stop." : ""}</p>
          </div>
        </div>

        <section className="rounded-lg bg-surface-2 p-3 text-xs text-muted-foreground" data-selectable>
          <dl className="grid grid-cols-[auto_1fr_auto_1fr] gap-x-4 gap-y-1">
            <dt>Format</dt>
            <dd className="text-foreground uppercase">{sound.format}</dd>
            <dt>Length</dt>
            <dd className="text-foreground">{formatDuration(sound.durationMs)}</dd>
            <dt>Sample rate</dt>
            <dd className="text-foreground">
              {(sound.sampleRate / 1000).toFixed(1)} kHz · {sound.channels === 1 ? "mono" : sound.channels === 2 ? "stereo" : `${sound.channels} ch`}
            </dd>
            <dt>Size</dt>
            <dd className="text-foreground">{formatBytes(sound.fileSize)}</dd>
            <dt>Original</dt>
            <dd className="truncate text-foreground">{sound.originalName}</dd>
            <dt>Played</dt>
            <dd className="text-foreground">
              {sound.playCount}× {sound.lastPlayedAt ? `· ${relativeTime(sound.lastPlayedAt)}` : ""}
            </dd>
            {sound.sourceUrl && (
              <>
                <dt>Source</dt>
                <dd className="col-span-3 truncate">
                  <button type="button" className="text-foreground underline-offset-2 hover:underline" onClick={() => void openUrl(sound.sourceUrl!)}>
                    {sound.sourceUrl}
                  </button>
                </dd>
              </>
            )}
          </dl>
        </section>

        <DialogFooter className="sm:justify-between">
          <Button
            type="button"
            variant="ghost"
            className="text-destructive hover:text-destructive"
            onClick={() => {
              onClose();
              void deleteSounds([sound.id]);
            }}
          >
            <Trash2 /> Delete
          </Button>
          <div className="flex gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={!!nameError || saving}>
              Save changes
            </Button>
          </div>
        </DialogFooter>
      </form>
    </DialogContent>
  );
}
