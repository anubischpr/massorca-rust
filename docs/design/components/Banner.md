# Banner

## Purpose

An inline strip at the top of a region that says something true about the region's state: most often that a host, terminal or runtime is connecting, reconnecting, disconnected or restored. It also carries other notices that belong to a place (a large diff, a pending credential cleanup, an external file change). It never floats; it pushes content.

Inventory ID: X-CN-01 (connection status banner; absorbs the SSH reconnect overlay, the remote runtime banner and the runtime disconnect toast). Inline notices in other screens use the same component.

## Anatomy

- Strip: full width of its container (the Conversation, a Work panel tab, a settings page, a dialog body), minimum height 36 px (`size.titlebar.windows`), padding `space.2` vertical and `space.3` horizontal, `radius.md` when inside a card or dialog and `radius.none` when it spans a region edge to edge, with a 1 px bottom border when edge to edge.
- Leading glyph 16 px: the status glyph (see variants), always with the word in the text.
- Text: a title in `body-strong` (optional) followed by the message in `body`, `text.primary`, wrapping to at most three lines, then an ellipsis with the full text in a tooltip.
- Detail line (optional, "Details" disclosure): `body-sm` `text.secondary`: host name, last contact time, error text, retry timer ("Retrying in 12 s").
- Actions: up to two buttons at `size.control.compact` (ghost or secondary), then an optional dismiss x icon button.
- Age: when the banner reports a last known state, the age shows in the text ("5 min ago", tabular) and refreshes every minute without an announcement.

## Variants and sizes

| Variant | Fill, border | Glyph and word | Typical use |
|---|---|---|---|
| info | `status.info.bg`, `status.info.border` | `info`, "Note" | Large diff, conflicts skipped, tips |
| success | `status.success.bg`, `status.success.border` | `circle-check`, "Done" or "Connected" | Reconnected, restored |
| warning | `status.warning.bg`, `status.warning.border` | `triangle-alert`, "Check" | Reconnecting, sign-in needed, out of date |
| danger | `status.danger.bg`, `status.danger.border` | `octagon-alert`, "Error" or "Failed" | A failure the user must see |
| unreachable | `agent.unreachable.bg`, `agent.unreachable.border` | dashed ring with a slash in `agent.unreachable`, "Can't reach" | Host unreachable |

Connection states (X-CN-01) map to variants:

| State | Variant | Message | Actions |
|---|---|---|---|
| Connecting | info, spinner after 200 ms | "Connecting to VPS" | none |
| Reconnecting | warning, spinner | "Reconnecting to VPS, attempt 3" | "Retry now" |
| Disconnected | unreachable | "VPS can't be reached. Showing the last known state from 5 min ago. Work that was running there may still be running." | "Retry", "Details" |
| Restored | success | "Reconnected to VPS" (fades after 5 seconds, dismissible) | none |
| Auth failed | warning | "VPS needs you to sign in" | "Sign in" |

Sizes: one size. In dense places (a terminal pane) the single-line form (36 px, no detail) is used; elsewhere the message may wrap. On Android the strip is full width under the top app bar, text `a-body`, actions 48 dp.

## States

| State | Visual | Tokens |
|---|---|---|
| default | As above | per variant |
| hover | Actions only | `fill.hover.<surface>` on ghost buttons |
| pressed | Actions only | `fill.pressed.<surface>` |
| focus-visible | Ring on the focused action or dismiss | `border.focus`, 2 px, offset 2 px |
| selected | Not applicable | not applicable |
| disabled | An action disabled while its request runs | `fill.disabled`, `text.disabled` |
| loading | Spinner glyph (connecting, reconnecting); a retry in flight shows a spinner in the Retry button | `agent.running` |
| empty | Not rendered when there is nothing to say | not applicable |
| error | The danger variant | `status.danger.fg` |
| expanded | "Details" open: the detail line shows | `text.secondary` |
| long content | Wraps to three lines, then an ellipsis; at narrow widths actions wrap under the text, aligned left | `text.primary` |
| disconnected host | This is the unreachable variant: last known state with its age. It is never worded as stopped, offline-and-done or failed | `agent.unreachable` |
| several notices | One banner per container: the highest priority shows (Disconnected, Reconnecting, Auth failed, Connecting, others) and a "+1 more" text button lists the rest | `text.secondary` |

## Behaviour (handoff)

- **Placement.** Inside its container at the top, above the content, pushing it down over `duration.moderate` with `ease.standard` (instant with reduced motion). It never overlays content. A connection banner sits at the top of every region that depends on the host (Conversation, terminal, Work panel tab, host card); the Sidebar host section and the status bar show the host state as glyphs, not banners.
- **Dismissal.** Connection banners are not dismissible while the state is true; a restored banner dismisses itself. Other banners have the x button; dismissal lasts until the state changes.
- **Keyboard.** Banners are not tab stops. Their buttons are reachable in reading order at the top of the region. Escape on a focused dismissible banner dismisses it.
- **Focus order.** Banner actions come before the content below them in the region.
- **Pointer.** Buttons only; the text is selectable.
- **Resizing.** Fills the container; wraps as above.
- **Motion.** Enter over `duration.moderate` with `ease.enter`; leave with `ease.exit`; the placement move uses `ease.standard`. A spinner uses `duration.spinner` and keeps turning with reduced motion. Colour changes between variants are instant.
- **Accessibility.** `role="status"` (polite) for connecting, reconnecting, restored and info; `role="alert"` for danger and for the first appearance of Disconnected. The glyph is decorative; the word is in the text. The age updates silently. Buttons keep their names. A restored banner is announced once.
- **Extension limits.** The Banner is an allowed component in extension views (kind and text, up to one action, an optional dismiss). Extensions cannot use the unreachable or connection variants, cannot imitate core connection messages (their name prefixes the text), and cannot place a banner in core regions.

## GPUI notes

- A flex row with a fixed minimum height; the message is a wrapping text element with a line clamp. Height animation clips the content (content keeps its final width). Draw the border as a quad edge, not a CSS-like border on all sides when edge to edge.
- The unreachable glyph is the shared AgentStatus element (dashed ring with slash) so it matches the sidebar and status bar.
- The age text is computed from a timestamp in the view and re-rendered once a minute by a single shared timer, not per banner.

## React Native notes (Android)

- A View under the top app bar, full width, 56 dp minimum when it has actions; text `a-body`, scaling to 200 percent (the message wraps, actions drop below). Actions 48 dp; dismiss is a 48 dp target. `accessibilityLiveRegion="polite"` (`"assertive"` for danger and first Disconnected). Respect "Remove animations": no height animation.
- Pending credential cleanup (M-ST-01) is a warning Banner at the top of the settings list.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Placement | Top of a region, pushing content | Under the top app bar, pushing content |
| Actions | `size.control.compact` | 48 dp |
| Detail | "Details" disclosure | Opens a sheet |
| Text | `body` | `a-body` |

## Copy

- Connection: "Connecting to VPS", "Reconnecting to VPS, attempt 3", "VPS can't be reached. Showing the last known state from 5 min ago. Work that was running there may still be running.", "Reconnected to VPS", "VPS needs you to sign in".
- Actions: "Retry", "Retry now", "Details", "Sign in", "Dismiss".
- Other: "This diff is large (12,431 lines). Show it anyway?", "Conflicted hunks are skipped. Resolve them in the merge view."
- Overflow: "+1 more".

## Token gaps and open points

Token gaps: none. The unreachable variant uses `agent.unreachable.bg` and `agent.unreachable.border`, so it reads at a glance next to the other four variants.

Open points:
- Automatic dismissal time for "Restored" (5 seconds proposed).
- The retry back-off and "attempt N" wording come from the transport layer; the banner only displays them.
