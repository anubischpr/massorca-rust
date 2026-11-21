# 0024: Icons and fonts are bundled into the binary

Status: accepted, 2026-11-20.

## Context

Handoff 2.1 and design-system 4.6: the interface font is Geist (400, 500,
600), the mono font JetBrains Mono (regular, italic, bold, bold italic), both
bundled as static files and never fetched. Icons are Lucide at 12, 16, 20 and
24 px. Agreement A94: agent icons are the real assets of the MassOrca app
(Claude, Codex, Antigravity, Gentle Shell, Gemini, OpenCode, Cursor); Gentle
Shell is always the rose, never the Pi mark. A26: the MassOrca logo is the
pink rose.

## Decision

- **Assets folder.** `crates/ui/assets/` holds `fonts/`, `icons/` (Lucide
  SVGs), `agents/` (agent icons) and `brand/` (the MassOrca logo). An
  `AssetSource` implementation embeds the folder with `include_bytes!` through
  a generated table, so the binary is self-contained and no path is read at
  run time.
- **Fonts.** The static TTF files from the official releases (Geist from
  `vercel/geist-font`, JetBrains Mono from `JetBrains/JetBrainsMono`), both
  SIL Open Font Licence 1.1, with their licence files next to them. They are
  registered with `cx.text_system().add_fonts()` at start-up. Missing glyphs
  fall back to the OS UI font through GPUI's fallback.
- **Lucide.** Only the icons the components use are copied, with Lucide's ISC
  licence file. Icons are an enum (`Icon::GitBranch`), so a missing icon is a
  compile error, not a blank square. SVGs are drawn with `svg()` and tinted
  with `text_color`, which matches Lucide's `currentColor` strokes.
- **Agent icons.** Taken as files from the MassOrca reference app, which the
  agreement requires: the Claude and Codex (OpenAI) marks as SVG files made
  from the exact path data in `status-bar/icons.tsx`; Antigravity, Gentle
  Shell, Gemini and Cursor as the PNGs from `src/shared/agent-icons/`; OpenCode
  from its glyph. Claude keeps its fixed `#D97757`; Codex follows the text
  colour; PNGs are drawn as supplied. Only assets are taken, never code.
- **One `AgentIcon` component** maps an `AgentKind` to its asset; an unknown
  agent uses the generic agent mark (A18).

## Alternatives

- **Load assets from disk next to the binary.** One more thing to install and
  to get wrong per OS.
- **An icon font.** Lucide's font loses stroke tinting control and adds a
  font fallback path.

## Consequences

- The binary grows by about 2 MB of fonts and icons.
- Adding an icon is one file plus one enum variant.
