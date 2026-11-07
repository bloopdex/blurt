import { shortcutParts } from "@/lib/format";
import { cn } from "@/lib/utils";

export function Kbd({ shortcut, className }: { shortcut: string; className?: string }) {
  return (
    <span className={cn("inline-flex shrink-0 items-center gap-0.5", className)} aria-label={`Shortcut ${shortcut}`}>
      {shortcutParts(shortcut).map((p, i) => (
        <kbd
          key={i}
          className="rounded border border-b-2 bg-surface-2 px-1 font-sans text-[10px] leading-4 font-medium text-muted-foreground"
        >
          {p}
        </kbd>
      ))}
    </span>
  );
}
