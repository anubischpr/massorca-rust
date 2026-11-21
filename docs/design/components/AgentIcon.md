# AgentIcon

The mark of a coding agent, shown wherever an agent is named: session rows, the Sidebar, the command palette Sessions group, the Model picker, composer chips, Session summary cards, delegation rows, usage rings, accounts, notifications and Android.

## Source

The real icons of the MassOrca app (pv96 reference, `D:\projects\massorca\.claude\worktrees\unified-custom-profile-build-547ea6`), copied exactly, never redrawn. The app's catalogue is `src/renderer/src/lib/agent-catalog.tsx` (`AgentIcon`), which maps each agent to its icon:

| Agent | Asset in the app | Kind | Colour |
|---|---|---|---|
| Claude | `src/renderer/src/components/status-bar/icons.tsx`, `ClaudeIcon` | SVG path, 24 viewBox | `#D97757`, fixed |
| Codex | `src/renderer/src/components/status-bar/icons.tsx`, `OpenAIIcon` | SVG path, 24 viewBox | `currentColor` (text colour), so it reads in both themes |
| Antigravity | `src/shared/agent-icons/antigravity.png` | PNG 64 x 64 | as supplied |
| Gentle Shell | `src/shared/agent-icons/gentle-shell.png` (`src/renderer/src/lib/agent-icon-gentle-shell.tsx`) | PNG 256 x 256, the rose | as supplied, both themes |
| Gemini | `src/shared/agent-icons/gemini.png` | PNG 64 x 64 | as supplied |
| OpenCode | `src/renderer/src/lib/agent-icon-glyphs.tsx`, `OpenCodeIcon` | SVG, 512 viewBox | `currentColor`, inner square at 28 percent |
| Cursor | `src/shared/agent-icons/cursor.png` | PNG 64 x 64 | as supplied |

Android uses the same files: `mobile/src/components/mobile-agent-icon-assets.ts` bundles the shared PNGs, and `mobile/src/components/AgentIcons.tsx` repeats the Claude and OpenAI paths.

## Rules

- Gentle Shell is the rose. It is never called "Pi" and never shown with the Pi mark; in the app, the `pi` agent with the `gentle-shell` runtime renders `GentleShellIcon`.
- The icon replaces the agent name at the start of titles and labels; the name stays as tooltip and accessible name (agreements A16, A17, A94).
- PNG marks keep the app's 2 px corner at 14 px (about 14 percent of the size).
- Sizes on screens: 12 (dense chips), 14 (desktop rows, chips), 16 (default), 20 (Android rows), 24 (Model picker rail), 32 (usage ring centre is 14 inside the ring).
- No letter monograms or generic glyphs for an agent that has an asset. An agent with no asset in the app falls back to the app's own `AgentLetterIcon` (a rounded square with the initial) and is listed in pending-owner-review.md.

## Accessibility

Role `img`, accessible name = agent name ("Claude", "Gentle Shell"). Decorative when the name is written next to it (`aria-hidden`).
