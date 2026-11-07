import { useEffect, useMemo, useState } from "react";
import { FileAudio, Headphones, Square, Upload, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { audioExtensions } from "@/lib/actions";
import { api } from "@/lib/api";
import { GroupIcon } from "@/lib/icons";
import { preview } from "@/lib/playback";
import { useApp } from "@/store/app";
import { useLibrary } from "@/store/library";

const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p;
const ext = (p: string) => fileName(p).split(".").pop()?.toLowerCase() ?? "";

export function ImportDialog() {
  const { pendingImport, setPendingImport, groups, importFiles } = useLibrary();
  const { view, voices } = useApp();
  const [files, setFiles] = useState<string[]>([]);
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [previewing, setPreviewing] = useState<{ path: string; voice: number } | null>(null);
  const categories = useMemo(() => groups.filter((g) => g.kind === "category").sort((a, b) => a.position - b.position), [groups]);

  useEffect(() => {
    if (!pendingImport) return;
    setFiles(pendingImport);
    setCategoryId(view.kind === "category" ? view.id : null);
  }, [pendingImport, view]);

  const supported = files.filter((f) => audioExtensions.includes(ext(f)));
  const unsupported = files.filter((f) => !audioExtensions.includes(ext(f)));
  const activePreview = previewing && voices.some((v) => v.voiceId === previewing.voice) ? previewing.path : null;

  const close = () => {
    if (previewing) void api.stopVoice(previewing.voice);
    setPreviewing(null);
    setPendingImport(null);
  };

  const togglePreview = async (path: string) => {
    if (previewing) await api.stopVoice(previewing.voice);
    if (activePreview === path) return setPreviewing(null);
    const voice = await preview(() => api.previewFile(path));
    setPreviewing(voice !== null ? { path, voice } : null);
  };

  const submit = async () => {
    const list = supported;
    close();
    await importFiles(list, categoryId);
  };

  return (
    <Dialog open={!!pendingImport} onOpenChange={(o) => !o && close()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Add {supported.length === 1 ? "a sound" : `${supported.length} sounds`}</DialogTitle>
          <DialogDescription>Files are copied into Blurt's library, so moving or deleting the originals won't break your board.</DialogDescription>
        </DialogHeader>
        <ul className="max-h-72 space-y-1 overflow-y-auto">
          {supported.map((f) => (
            <li key={f} className="flex items-center gap-3 rounded-md bg-surface-2 py-1.5 pr-1.5 pl-3">
              <FileAudio className="size-4 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 truncate text-sm" title={f}>
                {fileName(f)}
              </span>
              <Button variant="ghost" size="icon-xs" aria-label={`${activePreview === f ? "Stop preview of" : "Preview"} ${fileName(f)}`} onClick={() => void togglePreview(f)}>
                {activePreview === f ? <Square className="fill-current" /> : <Headphones />}
              </Button>
              <Button variant="ghost" size="icon-xs" aria-label={`Remove ${fileName(f)}`} onClick={() => setFiles((x) => x.filter((p) => p !== f))}>
                <X />
              </Button>
            </li>
          ))}
        </ul>
        {unsupported.length > 0 && (
          <p className="rounded-md border border-warn/30 bg-warn/10 px-3 py-2 text-xs text-warn">
            Skipping {unsupported.length} unsupported file{unsupported.length === 1 ? "" : "s"} ({unsupported.map(fileName).slice(0, 3).join(", ")}
            {unsupported.length > 3 ? "…" : ""}). Blurt plays MP3, WAV, OGG, FLAC, M4A/AAC and AIFF.
          </p>
        )}
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
        <DialogFooter>
          <Button variant="ghost" onClick={close}>
            Cancel
          </Button>
          <Button disabled={supported.length === 0} onClick={() => void submit()}>
            <Upload /> Add to board
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function DropOverlay({ visible }: { visible: boolean }) {
  if (!visible) return null;
  return (
    <div className="pointer-events-none fixed inset-0 z-50 grid place-items-center bg-background/80 backdrop-blur-sm">
      <div className="rounded-2xl border-2 border-dashed border-primary px-16 py-12 text-center">
        <Upload className="mx-auto mb-3 size-8 text-primary" />
        <p className="font-display text-xl font-bold">Drop to add sounds</p>
        <p className="mt-1 text-sm text-muted-foreground">MP3, WAV, OGG, FLAC, M4A/AAC, AIFF</p>
      </div>
    </div>
  );
}
