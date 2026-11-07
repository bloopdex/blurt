import {
  Bell,
  Briefcase,
  Coffee,
  Drum,
  Film,
  Flame,
  Folder,
  Gamepad2,
  Ghost,
  Heart,
  Laugh,
  Megaphone,
  Mic,
  Music,
  PartyPopper,
  Radio,
  Skull,
  Smile,
  Sparkles,
  Star,
  Swords,
  Trophy,
  Tv,
  Zap,
  type LucideIcon,
} from "lucide-react";

/** Icons offered for categories and collections; keys are what the database stores. */
export const groupIcons: Record<string, LucideIcon> = {
  folder: Folder,
  laugh: Laugh,
  zap: Zap,
  "gamepad-2": Gamepad2,
  music: Music,
  briefcase: Briefcase,
  film: Film,
  tv: Tv,
  star: Star,
  heart: Heart,
  flame: Flame,
  mic: Mic,
  radio: Radio,
  sparkles: Sparkles,
  ghost: Ghost,
  skull: Skull,
  "party-popper": PartyPopper,
  megaphone: Megaphone,
  bell: Bell,
  drum: Drum,
  smile: Smile,
  swords: Swords,
  trophy: Trophy,
  coffee: Coffee,
};

export const groupColors = [
  "#FF5B3A",
  "#F5B83D",
  "#B8F04A",
  "#2BC38A",
  "#3BA7F5",
  "#8B6CFF",
  "#E85BC7",
  "#8A8F9C",
];

export function GroupIcon({ name, className, color }: { name: string; className?: string; color?: string }) {
  const Icon = groupIcons[name] ?? Folder;
  return <Icon className={className} style={color ? { color } : undefined} aria-hidden="true" />;
}
