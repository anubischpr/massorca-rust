# Settings row and section

## Purpose

The layout and behaviour of one setting (a label, a helper line and a control) and of the section that groups them, for every settings page on desktop and the settings lists on Android. Extension settings sections use the same rows through contribution point 9.

Inventory IDs: D-ST-01 to D-ST-11 (desktop settings), M-ST-01 (Android settings menu), D-EX-04 (extension sections). The vault list is not a Settings row; a secret reference row links to it.

## Anatomy

**Section**
- Title in `title-sm` above the group, optional description in `body-sm` `text.secondary` (up to two lines), optional header action (a ghost button). Gap title to group `space.3`; gap between sections `space.8`.
- Group: `surface.raised`, `border.default`, `radius.lg`, `shadow.e1`. Rows inside are separated by `border.subtle` hairlines (never nested raised surfaces).
- Extension section: the extension's icon (16 px) and name appear in the header, after the title, in `caption` `text.tertiary`.
- The page is at most `layout.page.settings` (880 px) wide with `space.6` gutters.

**Row**
- Minimum height `size.row.settings.helper` (48 px) for a label with a helper line, `size.row.settings` (40 px) for a label alone; padding `space.3` vertical and `space.4` horizontal.
- Label column (left, flexible): label in `body` `text.primary` (`body-strong` is not used), helper in `body-sm` `text.secondary`, up to 3 lines, then an ellipsis with the full text in a tooltip. Inline chips after the label: "Restart required" (warning Badge), "Managed" (neutral), "Experimental" (info).
- Control column (right, minimum `layout.settings.control.min` 160 px, maximum `layout.settings.control.max` 320 px, right-aligned): the control for the row type.
- Modified marker: when the value differs from its default, a "Reset" icon button (rotate-ccw, `size.control.compact`, `text.secondary`, tooltip "Reset to default") sits left of the control.
- Status slot under the control or the helper: "Saving", "Saved" (check, `status.success.fg`) and error text (octagon-alert, `status.danger.fg`).

**Row types** (all from the allowed extension components list)

| Type | Control |
|---|---|
| Toggle | Switch |
| Text | TextField (single line); width up to 320 |
| Number | TextField with a stepper, unit in `caption` `text.tertiary` |
| Select | Select (dropdown); long lists use a Picker |
| Segmented | SegmentedControl (up to 4 options) |
| Slider | Slider with value label (font scale, zoom) |
| Keybinding | A Kbd badge that becomes a recorder on click |
| List | Compact list with Add and Remove controls |
| Secret reference | Vault link: secret name (`mono-sm`), scope Badge, link icon, and a "Manage in vault" text link. Never a value |
| Action | A Button ("Sign out", "Clear cache") in place of a control |
| Read-only | Value text with a copy icon button |
| Link | The whole row is a button with a chevron-right; opens a sub-page (the Android pattern) |

## Variants and sizes

| Variant | Difference |
|---|---|
| Default | Label left, control right. |
| Stacked | Label above control, full width. Used when the page is narrower than 560 px, for Text, List and Keybinding rows with long values, and on Android. |
| Compact density | Row padding `space.2`, minimum height 32. Density changes row height and padding only. |
| Android | Rows 56 dp (two-line 72 dp), touch targets 48 dp, a Switch inline at the right, other types open an editor (sheet or screen). |

## States

| State | Visual | Tokens |
|---|---|---|
| default | As anatomy | `surface.raised`, `text.primary`, `text.secondary` |
| hover | Only link rows and rows that are one big control fill; plain rows do not change | `fill.hover.raised` |
| pressed | Link rows | `fill.pressed.raised` |
| focus-visible | Ring on the control, never on the row | `border.focus`, 2 px, offset 2 px |
| selected | The row opened from search is outlined by a 2 px `accent.fg` bar on its left edge and `accent.subtle.raised` for 2 seconds | `accent.subtle.raised`, `accent.fg` |
| disabled | Control disabled (`fill.disabled`, `text.disabled`); the label stays `text.secondary` and the helper explains why ("Managed by your organisation", "Needs Claude to be signed in") in `text.tertiary` | `fill.disabled`, `text.disabled` |
| loading | The control is a Skeleton while an async value loads (for example the available models) | `fill.skeleton` |
| saving | After 200 ms a 12 px spinner appears at the control; the value stays | `agent.running` |
| saved | "Saved" with a check for 2 seconds after the store confirms | `status.success.fg` |
| error | Control border `status.danger.border`; message under it: "Enter a number from 1 to 10." The previous valid value stays in effect | `status.danger.fg` |
| modified | Reset button visible | `text.secondary` |
| restart required | Chip "Restart required" and a "Restart now" ghost button; the value applies after restart | `status.warning.bg`, `status.warning.fg` |
| empty | A List row with no items: "None yet" and an Add button | `text.tertiary` |
| long content | Label truncates with an ellipsis and never wraps (3.2); helper to three; a Text value scrolls inside the field; a Read-only value truncates in the middle with a tooltip | `text.secondary` |
| disconnected host | Host-scoped settings (D-ST-07 hosts, per-host agent settings) show the last known value dimmed with its age ("Last known 5 min ago"); the control is disabled and the helper says "Can't change while VPS is unreachable". App-level settings are unaffected | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Apply model.** Settings apply when changed. There is no page-level Save. Toggles, selects and segmented controls apply on change; text, number and keybinding rows apply on Enter or blur; Escape reverts the edit. Nothing says "Saved" before the store confirms it. A failed write returns the control to its previous value and shows the error.
- **Search.** The Settings page has a SearchField at the top. Matching rows and sections show; others hide; the match is highlighted with `match.bg` on the label text. Choosing a result scrolls to the row and uses the "selected" treatment.
- **Keyboard.** Tab moves between controls in reading order (label order). Clicking the label focuses or toggles its control. Space toggles a Switch; arrows operate Segmented, Radio and Slider; Enter on a Link row opens it. `Mod+,` opens Settings; `Mod+F` focuses settings search while the page has focus.
- **Focus order.** Settings navigation, then search, then the rows from top to bottom.
- **Pointer.** The control receives all input. Link rows are fully clickable. Right-click on a row offers "Copy setting name" and "Reset to default".
- **Resizing.** Rows switch to the stacked variant below 560 px. The page never scrolls horizontally.
- **Motion.** Control state changes use their own motion. The saved check fades out over `duration.base`. The search highlight bar fades after 2 seconds. Reduced motion: instant.
- **Accessibility.** Each row is a group named by its label, described by its helper; the control gets the label as its accessible name. A Switch is `role="switch"`. The reset button reads "Reset <label> to default". Errors are tied to the control and announced politely when they appear. "Saved" is announced once. Chips are part of the label text for screen readers ("Restart required").
- **Extension limits** (contribution point 9). A settings section from an extension has up to 40 rows, labels up to 60 characters, appears under the Extensions group and shows the extension's name. Allowed row types: toggle, text, number, select, keybinding, list, and secret reference. It may change its own settings and defaults only; it cannot edit other sections, restyle rows, open itself, or display or store a secret value (a secret reference row shows a vault link, and edits go through the Secret modal).

## GPUI notes

- A row is a flex row with a label column (`flex_1`) and a control slot; the slot hosts the shared control elements. Do not build custom controls inside the row.
- Settings values come from a typed settings store with defaults; "modified" is `value != default`, computed in the store, not the view. The store emits a confirmation event the row listens to for "Saved".
- Group rows with hairlines drawn as a bottom border on each row except the last; never nest raised surfaces.
- Search highlighting needs text runs with a background; reuse the Find bar's match run type.

## React Native notes (Android)

- A SectionList: section title rows, then 56 dp rows (72 dp when a helper line wraps). Switch rows toggle inline with the whole row as a 48 dp target. Select and text rows open a bottom sheet or a screen. The top-level settings menu (M-ST-01) is a list of Link rows grouped like the desktop sections; a pending credential cleanup notice is a Banner at the top. TalkBack: each row reads label, helper and value or state; "Reset" is an accessibility action.
- Respect system font scale up to 200 percent: rows use minimum height, and the control moves below the label when the label does not fit (the stacked variant).

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Layout | Label left, control right, grouped in a raised card | Full-width list rows, no card; Link rows to sub-screens |
| Editing | In the row | Switch inline; others in a sheet or screen |
| Search | Page search field | System-style search in the list (not in v1) |
| Keybinding row | Recorder | Not present |
| Extension sections | Yes | No |

## Copy

- Chips: "Restart required", "Managed", "Experimental".
- Status: "Saving", "Saved", "Reset to default", "Restart now".
- Errors: "Enter a number from 1 to 10.", "Could not save. The previous value is kept."
- Disabled reasons: "Managed by your organisation", "Needs Claude to be signed in", "Can't change while VPS is unreachable".
- Secret reference: "Manage in vault".
- Empty list: "None yet" with "Add".

## Token gaps and open points

Token gaps: none. Row heights are `size.row.settings` and `size.row.settings.helper`; the control column limits are `layout.settings.control.min` and `layout.settings.control.max`.

Open points:
- Auto-apply for every row type, with no Save button, is the proposal; some settings (account sign-in, host pairing) are actions and stay explicit.
- Settings search is assumed (D-ST-01 does not list it); the highlight uses `match.bg`.
- How organisation-managed settings are delivered ("Managed") is not defined.
