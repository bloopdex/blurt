import { useState } from "react";
import { Keyboard, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/Kbd";
import { eventToAccelerator } from "@/lib/format";
import { cn } from "@/lib/utils";

/** Click, press a combination, done. Esc cancels; the clear button removes the binding. */
export function ShortcutRecorder({
  value,
  onChange,
  label,
}: {
  value: string | null;
  onChange: (accelerator: string | null) => Promise<void> | void;
  label: string;
}) {
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);

  const onKeyDown = async (e: React.KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey) {
      setRecording(false);
      return;
    }
    const acc = eventToAccelerator(e.nativeEvent);
    if (!acc) return;
    setRecording(false);
    setBusy(true);
    await onChange(acc);
    setBusy(false);
  };

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        aria-label={recording ? `${label}: press the keys now, Escape to cancel` : `${label}: ${value ?? "not set"}. Click to record.`}
        onClick={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={(e) => void onKeyDown(e)}
        disabled={busy}
        className={cn(
          "flex h-9 min-w-48 items-center gap-2 rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring dark:bg-input/30",
          recording && "border-primary ring-2 ring-primary/30",
        )}
      >
        <Keyboard className="size-4 text-muted-foreground" />
        {recording ? (
          <span className="text-primary">Press keys…</span>
        ) : value ? (
          <Kbd shortcut={value} />
        ) : (
          <span className="text-muted-foreground">Click to set</span>
        )}
      </button>
      {value && !recording && (
        <Button type="button" variant="ghost" size="icon-sm" aria-label={`Remove ${label}`} onClick={() => void onChange(null)}>
          <X />
        </Button>
      )}
    </div>
  );
}
