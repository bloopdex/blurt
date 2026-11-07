# Blurt brand

**Name.** *Blurt*: to say something suddenly, out loud. That's exactly what a soundboard does in a call. It's short, easy to say, and verb-friendly ("blurt it out"). Alternatives considered: *Hotmic* (taken by existing apps), *Cuebox* and *Quip* (generic), *Airhorn* (too narrow).

**Personality.** Playful but precise. The copy is plain and direct ("Only you can hear this", "Mic live"), never cute when something is broken.

**Mark.** A speech bubble mid-shout: three level bars inside a rounded bubble with a tail. It reads at 16 px (tray), and the bars double as an equaliser motif in the UI. Sources: `design/logo.svg` (mark), `design/icon.svg` (app icon on an ink squircle). Regenerate all icon sizes with `pnpm tauri icon design/icon.svg`.

**Colour.**

| Token | Dark | Light | Use |
|---|---|---|---|
| Blurt Coral (primary) | `#FF5B3A` | `#E8492A` | brand, play, progress, focus |
| Ink (background) | `#0D0E11` | `#F6F5F2` (warm paper) | canvas |
| Surface | `#131519` / `#1A1C22` | `#FFFFFF` / `#EFEEE9` | sidebar, cards, inputs |
| Live Lime | `#B8F04A` | `#4C9A12` | "mic live", success |
| Amber | `#F5B83D` | `#B97D06` | warnings, favourites |
| Red | `#F04F5F` | `#D93848` | errors, destructive |

Category colours: coral, amber, lime, green `#2BC38A`, blue `#3BA7F5`, violet `#8B6CFF`, pink `#E85BC7`, grey `#8A8F9C`.

**Type.** *Bricolage Grotesque* (display: wordmark, titles) for character; *Inter* (UI) for legibility, with tabular numbers for times. Both are bundled (OFL), so the app works offline.
