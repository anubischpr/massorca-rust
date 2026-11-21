# Progress

## Purpose

Shows how far an operation has got, or that it is working, in three forms: a determinate bar, an indeterminate bar, and a spinner for short or unknown waits. Usage meters are a different component (UsageRing, D-SH-15 and D-AC-06) and are not drawn with this one.

Inventory IDs: none of its own. Used by D-WS-15 (workspace creation progress), uploads and file transfers, the Checks panel, the busy states of Buttons and Dialogs (spinner), and the remote host connection spinner (after 200 ms).

## Anatomy

- Bar: height `size.progress.height` (4 px; 4 dp on Android), radius `radius.full`, track `progress.track` with `border.subtle` 1 px, fill `accent.default`.
- Spinner: `size.icon.sm` (16 px) on desktop, `size.icon.lg` (24 dp) on Android; an arc of 270 degrees in the fill colour, stroke 2 px.
- Label (optional): `caption` (11 / 16 / 400) in `text.secondary`, tabular figures, above or beside the bar. Example: "62%".
- Name (required): the thing in progress, as text, not only as the bar ("Creating worktree oppm-api").

## Variants and sizes

| Variant | Use |
|---|---|
| determinate bar | a known total: uploads, file copies, a run of N steps |
| indeterminate bar | a known task of unknown length |
| spinner | a short wait on a button, a row or a host (shows after 200 ms) |
| step list | not a Progress; use a list of steps with their status glyphs |

Sizes: bar 4 px, spinner 16 px. Android: bar 4 dp, spinner 24 dp.

## States

| State | Rendering |
|---|---|
| determinate | fill width equals the value; label shows the percentage in tabular figures |
| indeterminate | a 30 percent segment travels across the track in `duration.spinner` with `ease.linear` |
| spinner | arc turns in `duration.spinner` with `ease.linear`; appears after 200 ms |
| complete | fill at 100 percent, then the item leaves or shows its success status (glyph `circle-check` and the word "Done") |
| error | fill stops at the last value and turns `status.danger.solid`; label "Failed at 62%" with glyph `octagon-alert` and the word |
| paused | fill stays, label says "Paused" in `text.secondary` |
| empty | not applicable |
| loading | a spinner appears after 200 ms; nothing for the first 100 ms |
| disabled | `text.disabled` label; no fill colour change |
| disconnected host | the bar stops and the label reads "Connection lost. Last known 62%, 5 min ago" |

## Behaviour (handoff)

- Keyboard: not focusable. Cancel, when offered, is a separate Button next to it.
- Pointer: none on the bar. A cancel control is a separate IconButton with a tooltip.
- Resizing: the bar fills its container; min width 80 px, below that the label goes first.
- Motion: determinate width changes in `duration.base` with `ease.standard`; indeterminate travel and spinner `duration.spinner` with `ease.linear`. Reduced motion: spinner keeps turning (functional); the indeterminate bar becomes a static half-filled track with the word "Working".
- Accessibility: role `progressbar` with `aria-valuenow`, `aria-valuemin` and `aria-valuemax` for determinate; no value for indeterminate. Name is the thing in progress. Announce start and completion once, politely, not every step.
- Extensions: may show a determinate bar or a spinner in an extension view (allowed list). Cannot change the colour or the height.

## GPUI notes

- Two rounded `div`s for track and fill; fill width is a fraction of the track.
- Spinner is an SVG arc rotated over `duration.spinner`; the animation runs on the element, not on the render loop.
- No shadow on the bar.

## React Native notes

- `ProgressBarAndroid`-style bar drawn with `View`s for consistent sizing; `accessibilityRole="progressbar"` with `accessibilityValue`.
- Spinner is `ActivityIndicator` coloured `accent.default`; delayed 200 ms with a small timer.

## Desktop vs Android

- 4 px bar on both; 16 px spinner on desktop against 24 dp on Android.
- Android shows progress in the sheet or the screen header, not in a floating panel.

## Copy

- Name the thing: "Creating worktree oppm-api", "Uploading 3 files".
- Progress words are progressive: "Creating", "Uploading"; the finished word comes after it is true: "Created".
- Percent with no decimals.

## Token gaps and open points

Token gaps: none. The bar height is `size.progress.height`; the track stays `surface.sunken` with `border.subtle`, which needs no token of its own.

Open points:
- Whether the indeterminate travel should stop after a time with a message for long waits. Spec says yes, after 30 s show the step name, not a percentage.
