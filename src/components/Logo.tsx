import { cn } from "@/lib/utils";

/** The Blurt mark: a speech bubble that's mid-shout (three level bars). */
export function LogoMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 64 64" className={cn("size-8", className)} aria-hidden="true">
      <path
        fill="#FF5B3A"
        d="M20 6h24c8.837 0 16 7.163 16 16v12c0 8.837-7.163 16-16 16H27.5l-13.8 10.35c-1.32.99-3.2-.12-2.95-1.75L12.6 48.7C7.5 46 4 40.4 4 34V22C4 13.163 11.163 6 20 6Z"
      />
      <rect x="19" y="21" width="6" height="15" rx="3" fill="#0D0E11" />
      <rect x="29" y="15" width="6" height="27" rx="3" fill="#0D0E11" />
      <rect x="39" y="19" width="6" height="19" rx="3" fill="#0D0E11" />
    </svg>
  );
}

export function Wordmark({ className }: { className?: string }) {
  return (
    <span className={cn("font-display text-[19px] font-extrabold tracking-[-0.03em]", className)}>
      blurt
    </span>
  );
}
