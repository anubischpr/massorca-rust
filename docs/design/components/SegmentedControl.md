# SegmentedControl

## Purpose

One row of two to four mutually exclusive options, shown as joined segments, to switch a view or a setting in place. Not for moving between regions or pages, and not for more than four options (use Select).

Inventory IDs: none of its own. Used by D-ED-11 (Names or Content search toggle), D-DF-01 (rendered or source, split or unified), D-AG-23 (Day, Week, Month).

## Anatomy

- Track: `surface.raised` fill, `border.strong` 1 px outline (it identifies a control), radius `radius.md`, inner padding `space.half` (2 px). Outlined is for forms, dialogs and panels.
- Sunken variant (A30, A31): no border; the track is `surface.sunken`, radius `radius.lg` (8), inner padding `space.half`; the active option is `fill.selected.raised` with `text.primary` and weight 600, the others `text.secondary`; hover `fill.hover.raised`. Used for every switch inside a composer or toolbar: "Chat | Terminal", the Tasks "Workspaces / Bot Teams" switch and the Model picker's "Standard | Fast". The track fill and the selected fill identify the control, so it needs no outline.
- Segment: radius `radius.sm` (4 px, the track radius minus the 2 px padding). Height is the track height minus 2 × 2 px.
- Label: style `label` (12 / 16 / 500). Android: 14 / 20 / 500.
- Icon (optional): Lucide, `size.icon.sm` (16 px), gap `space.1` to the label.
- Dividers between segments: none. Segments separate by fill only. On Android they form one target group (5.4): they touch with no 8 dp gap, each keeping a 48 dp hit area.

## Variants and sizes

| Variant | Content | Rule |
|---|---|---|
| text (default) | label only | Default for two to four short options |
| icon and text | 16 px icon, gap `space.1`, label | When the icon adds meaning |
| icon only | icon only | Every segment needs a tooltip and a name. Max four. |

| Size | Track height | Label |
|---|---|---|
| compact | `size.control.compact` (24) | `label` |
| default | `size.control.default` (28) | `label` |
| comfortable | `size.control.comfortable` (32) | `body` |

Selected style: quiet by default (`fill.selected.raised` with `text.primary`). The strong style (`accent.default` fill with `text.on-accent`) is used once per view at most, for a primary mode choice inside a panel.

Android: track 44 dp, segments at least 48 dp wide, equal widths.

## States

| State | Rendering |
|---|---|
| default | unselected label `text.secondary`; track `border.strong` |
| hover | `fill.hover.raised` on the segment, label `text.primary` |
| pressed | `fill.pressed.raised` on the segment |
| focus-visible | ring `border.focus`, 2 px, on the focused segment, inset by 2 px so it stays inside the track |
| selected | `fill.selected.raised` (quiet) or `accent.default` (strong); label `text.primary` or `text.on-accent`; `aria-checked="true"` |
| disabled | track fill `fill.disabled`; labels `text.disabled`; `aria-disabled="true"`. A disabled segment keeps the same rule. |
| loading | not applicable. A change that loads data shows the content's Skeleton, not a spinner on the control. |
| empty | not applicable |
| error | not applicable on the control. A failed change reverts the selection and the region shows its own error. |
| long content | labels truncate with an ellipsis, never wrap. Segments share the width equally until they reach a 64 px floor, then truncate. |
| disconnected host | when the setting lives on a host, all segments are disabled and the reason is visible text under the control |

## Behaviour (handoff)

- Keyboard: the group is one tab stop. Left and Right move focus and select at once (selection follows focus). Home and End go to the first and last segment. Enter and Space select the focused segment.
- Focus order: in the order of its region.
- Pointer: click selects on press. No drag.
- Resizing: segments are equal width when they fit; below that they are content width; then they truncate.
- Motion: fill colour step in `duration.fast` with `ease.standard`. No sliding indicator. Reduced motion: instant.
- Accessibility: role `radiogroup` with `radio` segments and `aria-checked`. Name is required, from a visible label outside the control or `aria-label`. The selection is announced by the content change, not by a separate live region.
- Extensions: may use the control in extension views (allowed list). Strong style is not allowed to extensions. Labels max 16 characters; max four segments.

## GPUI notes

- Track is a `div` with a 1 px border and a 2 px inner gap; segments are flex children with the state fill set precomputed.
- Keyboard handled by the track element (arrow, Home, End); each segment is focusable only through the group.
- Selection and focus are one state here, so the fill follows keyboard focus immediately.
- No animated indicator. Keeps the GPUI render path simple.

## React Native notes

- A row `View`; each segment a `Pressable` with `accessibilityRole="radio"` and `accessibilityState={{ checked, disabled }}`.
- Segment visual 44 dp high, hit area 48 dp. Equal `flex`.
- Label 14 sp / 20 / 500 with font scaling up to 200 percent; the track uses `minHeight`.
- TalkBack reads "Names, selected, 1 of 2".

## Desktop vs Android

- 28 px track against a 44 dp track.
- Hover and pressed fills against ripple.
- On Android a control with more than three segments moves to a Sheet with radio rows.

## Copy

- One or two words per segment, sentence case: "Names", "Content"; "Split", "Unified"; "Day", "Week", "Month".
- The control name says what it chooses: "Search in", "View".

## Token gaps and open points

Token gaps: none.

Open points:
- Whether the Android three-option rule should become a Sheet or stay inline. Spec keeps it inline up to three.
