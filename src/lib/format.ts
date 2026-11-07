export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 100) / 10);
  if (total < 10) return `${total.toFixed(1)}s`;
  const m = Math.floor(total / 60);
  const s = Math.floor(total % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function formatClock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(total / 60)}:${(total % 60).toString().padStart(2, "0")}`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function relativeTime(ts: number, now = Date.now()): string {
  const s = Math.round((now - ts) / 1000);
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  const d = Math.round(h / 24);
  if (d < 30) return `${d} d ago`;
  return new Date(ts).toLocaleDateString();
}

const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform);

const keyLabels: Record<string, string> = {
  Period: ".",
  Comma: ",",
  Slash: "/",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Minus: "-",
  Equal: "=",
  Backquote: "`",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Escape: "Esc",
  Backspace: "⌫",
  Enter: "↵",
  Space: "Space",
};

/** "control+alt+Digit1" → ["Ctrl", "Alt", "1"] (or ⌃ ⌥ 1 on macOS). */
export function shortcutParts(canonical: string): string[] {
  return canonical.split("+").map((part) => {
    const p = part.toLowerCase();
    if (p === "control" || p === "ctrl") return isMac ? "⌃" : "Ctrl";
    if (p === "alt" || p === "option") return isMac ? "⌥" : "Alt";
    if (p === "shift") return isMac ? "⇧" : "Shift";
    if (p === "super" || p === "command" || p === "cmd" || p === "meta") return isMac ? "⌘" : "Win";
    if (p === "commandorcontrol" || p === "cmdorctrl") return isMac ? "⌘" : "Ctrl";
    if (part.startsWith("Digit")) return part.slice(5);
    if (part.startsWith("Key")) return part.slice(3);
    if (part.startsWith("Numpad")) return `Num ${part.slice(6)}`;
    return keyLabels[part] ?? part;
  });
}

/** KeyboardEvent → Tauri accelerator using physical key codes, or null while only modifiers are held. */
type KeyLike = Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

export function eventToAccelerator(e: KeyLike): string | null {
  const modifierCodes = ["Control", "Alt", "Shift", "Meta", "OS"];
  if (modifierCodes.some((m) => e.key === m || e.code.startsWith(m))) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Control");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  parts.push(e.code);
  return parts.join("+");
}
