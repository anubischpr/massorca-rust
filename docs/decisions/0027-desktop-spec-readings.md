# 0027: Readings of the design where specs disagree or GPUI cannot follow

Status: accepted, 2026-11-21. Updated as components land.

## Context

The working rule is that agreements and the later artboard win over a
component spec. While building the first components and the shell, a few
places had two readings, or asked for something GPUI cannot draw. Each is
listed here once so a reviewer can find the choice and change it in one
place.

## Readings taken

| Where | The two readings | Chosen | Why |
|---|---|---|---|
| TextField focus border (X33) | TextField.md: 2 px outside the field; handoff 3.3.7 and the GPUI note: inside, in place | Inside, padding compensated so text does not move | Handoff is later and the outside ring is clipped in scroll areas |
| ListRow and SidebarRow selected bar | Component specs: `accent.fg`; handoff 3.6.1: `accent.default` | `accent.fg` | Both component specs and the artboards agree |
| RailItem active icon | Anatomy: `text.primary`; States table: `accent.fg` | `text.primary` | The artboard (`sh-empty-app`) draws it that way |
| Rail active bar position | Artboard: 6 px left of the button (x = 0 of the rail); RailItem.md silent | At the rail edge | Same place as the artboard |
| Status bar item gap | Artboard 14 px; StatusBar.md 12 px | 12 px (`space.3`) | A token value; the artboard copy lags the spec |
| Region toggles in the Title bar | RegionToggle.md list; D-SH-03 list | D-SH-03 | Screen spec is the later reading |
| Collapse thresholds (X1) | FrameCanvas.md Conversation minimum 360; thresholds assume 480 | Thresholds 1528 / 1288 / 1048 / 848 | design-system 5.1 and P8 |
| Skeleton delay | Handoff row 200 ms; Skeleton.md and design-system 4.8: 100 ms | 100 ms (`SKELETON_DELAY_MS`) | 4.8 is the resolved constant |
| Agent marks when dimmed | Specs: `text.disabled` mark; P32: full colour | Full colour | P32, and GPUI cannot tint PNG images |
| Overlays inside a dialog | Popover `z.popover` (40) is below `z.dialog` (50) | Overlays take a layer; inside a dialog they paint above it | Otherwise a Select in a dialog paints under the scrim |
| PlanView progress | "N of M steps" counts only Done; or Done and Skipped | Done and Skipped | A skipped step is finished; the total is every step |
| PlanView done glyph | PlanView.md: check circle in `status.success.fg`; AgentStatus: ring with check in `agent.done` | AgentStatus | One done mark across the app |
| PlanView "Jump to current step" | Only when the step is out of view; always | Always, while a current step exists | Visibility of a row inside a list is not observable here yet |
| DelegationTree long child lists | "Show more" past 12 shows 8 (rule); the example "Show 4 more" | First 8, then "Show N more" | The rule; the example contradicts it |
| DelegationTree depth | Unlimited; spec fold | Rows below depth 3 fold into "+N deeper" | Spec fold, 28 px indent per level (icon plus gap) |
| SessionSummaryCard icons | Spec silent for Child sessions and Plan | `git-branch`, `list` | Closest Lucide meaning |
| Bot Teams characters | Team character art; agent icons | Agent icons | No character assets yet |
| Composer on an unreachable host | Composer.md behaviour: the message queues; state table: it does not | State table: the message stays in the composer, Send is disabled, nothing is queued | Owner decision, 2026-11-21; Composer.md behaviour section corrected |

## What GPUI cannot do here (at the pinned commit)

- **Letter spacing.** No tracking in GPUI text styles: the `overline` style's
  0.04 em is dropped.
- **Transforms on boxes.** Only SVG elements take transforms, so the dialog's
  0.98 to 1 scale is not drawn; dialogs fade only. The mode-switch morph
  scales and rotates SVG icons, which works.
- **Style transitions.** Hover and pressed fills step instantly rather than
  over `duration.fast`.
- **Repeating animations under reduced motion.** GPUI freezes them, but
  spinners must keep turning (handoff 2.6), so spinners compute their angle
  from a frame clock.
- **Selectable text.** Error details offer a Copy button instead of a
  selectable mono block.
- **Tinting images.** PNG agent marks are drawn as supplied.
- **Rows of different heights in a virtual list.** `uniform_list` takes one
  row height, so DiffView hunk and collapsed rows are 20 px instead of 24 px
  and inline note cards are not drawn between lines.
- **Live regions and sort state.** No live-region politeness and no
  `aria-sort`: announcements use `Role::Status` (or `Role::Alert` for
  errors) and a table header carries "sorted ascending" in its label.
- **Animated roots.** `AnimationElement` forwards no accessibility role, so a
  role sits on a plain wrapper around an animated root.

## Literal values without a token

Claude's `#D97757` (A94), the 14 px agent mark beside a name (A17), Card's
320 px body cap (not enforced). These should become tokens in a later pass
of `docs/design/tokens.json`.

## Consequences

When the owner prefers the other reading of any row, the change is local to
the component named in it.
