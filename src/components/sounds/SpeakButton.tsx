import { useState } from "react";
import { Loader2, MessageSquareText } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { api, errorMessage } from "@/lib/api";
import { playSound } from "@/lib/playback";
import { useLibrary } from "@/store/library";

const DEFAULT = "__default";

/** Type a line, the system voice says it into the mic; it's kept as a sound for replays and hotkeys. */
export function SpeakButton() {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [voices, setVoices] = useState<string[] | null>(null);
  const [voice, setVoice] = useState(DEFAULT);
  const [busy, setBusy] = useState(false);

  const speak = async () => {
    setBusy(true);
    try {
      const s = await api.textToSpeech(text, voice === DEFAULT ? null : voice);
      useLibrary.getState().upsert(s);
      await playSound(s.id);
      setText("");
      setOpen(false);
    } catch (e) {
      toast.error("Couldn't speak that", { description: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (o && !voices) api.ttsVoices().then(setVoices, () => setVoices([]));
      }}
    >
      <PopoverTrigger asChild>
        <Button variant="outline" className="h-9">
          <MessageSquareText /> Speak
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 space-y-3">
        <p className="text-sm font-medium">Text to speech into your mic</p>
        <Textarea
          value={text}
          maxLength={500}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && text.trim()) {
              e.preventDefault();
              void speak();
            }
          }}
          placeholder="Type something, press Enter"
          aria-label="Text to speak"
          rows={3}
        />
        {!!voices?.length && (
          <Select value={voice} onValueChange={setVoice}>
            <SelectTrigger className="w-full" aria-label="Voice">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={DEFAULT}>Default voice</SelectItem>
              {voices.map((v) => (
                <SelectItem key={v} value={v}>
                  {v}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
        <Button className="w-full" disabled={busy || !text.trim()} onClick={() => void speak()}>
          {busy ? <Loader2 className="animate-spin" /> : <MessageSquareText />} Speak into mic
        </Button>
        <p className="text-xs text-muted-foreground">Each line is saved as a sound, so you can replay it or give it a shortcut.</p>
      </PopoverContent>
    </Popover>
  );
}
