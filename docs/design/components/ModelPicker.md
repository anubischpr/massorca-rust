# Model picker

## Purpose

The one selector for agent, model, reasoning effort and speed in every composer: the start page, Tasks and every session. A chip in the Composer's bottom row opens a three-column popover. Design follows the owner's requirement and two reference images (2026-11-12).

Inventory ID: X-AG-02. The access mode picker (D-AG-04) is the Composer's own access chip, beside this one (owner-approved composers v47 and v50, agreement A33; it supersedes the access part of A27). Extension entries: contribution point 12 (D-SL-10). Android shows the same parts as a sheet.

## Anatomy

**Trigger chip** (in the Composer bottom row, right group, before Send)

- Height `size.control.compact` (24 px), pill shape `radius.full` (A29), horizontal padding `space.2`, no border and no fill at rest (A12). Content: the agent's own icon 16 px (full colour; the generic agent mark only when the agent registers none, never a letter), model name in `label` (`text.primary`), effort in `label` (`text.secondary`), chevron-down 12 px (`text.tertiary`). Example: "Opus 5.5 Medium".
- Gap `space.1` between parts. The model name truncates first (maximum chip width `layout.model-picker.chip.max-width`, 220 px); the effort never truncates.

**Popover** (opens above the trigger)

- Surface `surface.overlay`, `border.default`, `radius.xl` (12), `shadow.e2`. Width `layout.model-picker.width` (560 px): agent rail `layout.model-picker.provider-column` (48 px), model column `layout.model-picker.model-column` (296 px), reasoning column `layout.model-picker.reasoning-column` (216 px). Height follows content, at most `layout.model-picker.max-height` (420 px). Right edge aligned to the trigger's right edge, `space.2` above it; clamped to the window with `space.2` margin. Flips below the trigger when there is no room above (the start page, where the Composer sits mid-screen).
- Column dividers and the footer divider: `border.subtle`.

**Agent rail** (left column)

- Favourites button (star icon, 20 px) on top, then a `border.subtle` divider, then one button per agent or provider (brand mark 20 px). Buttons 36 px square, `radius.md`, gap `space.1`, padding `space.1`.
- Agent marks are drawn in full colour, the same icon as in the usage rings and the trigger chip (A18); the Favourites star is `text.secondary`. Hover `fill.hover.overlay`; active (current view) `fill.selected.overlay` plus a 2 px `accent.fg` bar on the rail's right edge beside the button, 20 px tall (as in the reference).
- Agents that are signed out or missing are dimmed (`text.tertiary`) and say why in their tooltip.

**Model column** (middle)

- Search field on top: height `size.control.comfortable` (32), search icon 16 px `text.tertiary`, placeholder "Search models" in `body` `text.tertiary`, a 1 px `border.subtle` underline at rest. When focused the underline becomes 2 px `border.focus` (the owner's accent underline; the colour is the focus ring colour so it stays at 3:1).
- Model rows: height `size.control.comfortable` (32), `radius.md`, padding `space.2`. Contents: provider icon 16 px, name in `body` (`body-strong` when it is the current model), favourite star icon button (24 px, outline) shown on hover and focus only (filled `text.primary` when the model is a favourite), and a shortcut badge (Kbd at `size.kbd.height`, `mono-sm`, `surface.sunken`, `radius.sm`) with `Mod+1` to `Mod+4` on the first four rows.
- Hover `fill.hover.overlay`; keyboard-active `fill.selected.overlay`; current model: `body-strong` plus a 2 px `accent.fg` bar on the row's left edge.
- "Show more" (text button, `body`, `text.secondary`) under the four featured rows; it expands the list to every model of the agent and becomes "Show fewer".

**Reasoning column** (right)

- Header row: "Reasoning" (`label`, `text.secondary`) on the left and the current model's name (`caption`, `text.tertiary`) on the right, with a `border.subtle` underline.
- **Reasoning slider (A98, approved 2026-11-25; artboard `ws-reasoning-slider`).** It replaces the list of effort rows. The column is 216 px wide and 304 px high, with padding 8 px top, 12 px sides and 10 px bottom; the popup does not grow.
  - **Level name:** 8 px under the header, the selected level in `title` (16 / 24 / 600). Its colour is the agent brand colour that reads best on the surface (see Colour). At the top level a 14 px Lucide `sparkles` glyph (stroke, `currentColor`) follows it; its paths are in the artboard `ws-reasoning-slider` (`data-icon="lucide:sparkles"`).
  - **Agent and model:** "Claude · Opus 5.5" in `caption`, `text.tertiary`.
  - **Bar:** 10 px below the name, a vertical track 28 px wide, filling the rest of the column (about 208 px), radius `radius.full`. Track: `surface.sunken` with no border (owner, 2026-11-25). It is a slider (`role="slider"`, vertical) with one tab stop.
  - **Stops:** one per declared level, 2 to 5. The first and last sit 14 px from the bar ends, the rest evenly between. The spacing is computed from the track height that the 304 px column leaves, not fixed. Each stop is a 4 px dot in `text.tertiary` at 45 percent; stops at or below the selected one turn white at 70 percent.
  - **Fill:** from the bottom to the selected stop plus 14 px, radius `radius.full`. Linear gradient from bottom to top: the agent's first brand colour to its second.
  - **Thumb:** a 22 px circle centred exactly on the track's vertical axis (3 px from each side of the 28 px track) and on the selected stop, fill `slider.thumb`, a 1 px ring (`border.default` at 50 percent) and `shadow.e1`.
  - **Labels:** the level names to the right of the bar, 14 px from it, centred on their stops, in `body` `text.tertiary`; the selected one in `body-strong` `text.primary`. Each label is a click target.
  - **No slider:** a model with no levels or one level shows "This model has no reasoning levels" (`body-sm`, `text.tertiary`).
  - **Colour:** the brand colours are the agent package's `listing.banner.colors` (first and second), the same values its Marketplace banner uses: Claude #D97757 to #B5532F, Codex #4B5563 to #9CA3AF, Antigravity #4285F4 to #34A853, Gentle Shell #FF2E8A to #FF8CC2. An agent that declares one colour uses it at both ends; one that declares none uses `accent.default` to `accent.fg`. The level name takes whichever of the two colours has the higher contrast on `surface.overlay`.
  - **Below the top level:** the gradient fill only. No particles and no glow.
  - **Top level (the last stop):**
    - **Particles:** 46 white dots inside the fill, radius 0.7 to 2 px, rising 4 to 14 px per second, twinkling between 35 and 100 percent opacity on 2.6 to 7.9 s cycles; a dot that leaves the top re-enters at the bottom.
    - **Glow:** a halo 6 px outside the bar, the second brand colour at 55 percent, blurred 6 px. It follows the bar's rounded outline (radius 20 px), never a rectangle.
    - **Thumb ring:** 4 px, the second brand colour at 45 percent, plus a 16 px glow in that colour.
    - **Shimmer:** a 46 px band of white at 55 percent (transparent at both ends) climbs the fill from bottom to top every 1.6 s, `ease.linear`, repeating while the picker is open.
    - **Clipping (owner, 2026-11-25):** the particles and the shimmer band are drawn only inside the fill, clipped to its rounded shape (`radius.full`), including while the band enters at the bottom and leaves at the top. No square corner shows at any point of the animation. In GPUI, mask the effects layer to the fill's rounded rectangle, not to its bounds.
    - **Sparkle:** the `sparkles` glyph fades in next to the level name.
    - **Fades:** the effects fade in over `duration.moderate` (200 ms) and the halo over `duration.slow` (300 ms).
  - **Motion:** after a change, the thumb and the fill move to the new stop in one bounded transition, `duration.slider-move` (220 ms) with `ease.standard`. Below the top level nothing loops: no particles, no shimmer and no other repeating animation run. The only looping animations are the particles and the shimmer at the top level.
  - **Reduced motion** (system setting or MassOrca's own):
    - The top level shows the same frame held still: particles drawn once where they are, the halo, the thumb ring and the sparkle, and the shimmer band resting at the top of the fill.
    - The thumb and fill jump to a new stop with no transition.
    - Below the top level nothing changes.
  - **Input:** click or drag anywhere on the bar snaps to the nearest stop; a click on a label selects it. None of these closes the popover. Up and Right raise the level, Down and Left lower it, Home and End jump to the ends, Page Up and Page Down jump to the ends. The value text is "Medium, 2 of 5".
  - **Tokens:**
    - Existing: `surface.sunken`, `border.default`, `radius.full`, `shadow.e1`, `title`, `body`, `body-strong`, `caption`, `text.primary`, `text.tertiary`, `accent.default`, `accent.fg`, `duration.moderate`, `duration.slow`, `ease.standard`, `ease.linear`.
    - New (not yet in tokens.json; added by the session that builds it): `size.reasoning-slider.track` = 28 px, `size.reasoning-slider.thumb` = 22 px, `slider.thumb` = #f4f4f7 dark / #ffffff light, `duration.slider-shimmer` = 1600 ms, and `duration.slider-move` = 220 ms (no existing duration is 220 ms; `duration.moderate` is 200 ms).
  - **Android:** the same column inside the Model picker sheet. The bar is 32 dp wide, the thumb 26 dp, the stop spacing computed from the bar height as on desktop, and the labels in `body` (14 sp). The colours and effects are the same.

**Footer** (height 44 = control height 28 + `space.2` above and below, top `border.subtle`)

- Left: SegmentedControl "Standard | Fast", sunken variant (no border, `surface.sunken` track; the Fast segment carries a zap icon 12 px). The footer has no access control: access is the Composer's access chip, so the picker never offers a second one.

## Variants and sizes

| Variant | Difference |
|---|---|
| Desktop | Popover, as above. |
| Narrow window (less than 592 px available) | Width becomes available width minus `space.4`. Below 480 px the reasoning column moves under the model list and the whole popover scrolls; rows stay one per line. |
| Android | Bottom sheet, see React Native notes. |

Trigger chip density follows the Composer row: `size.control.default` (28) in default density, `size.control.comfortable` (32) in comfortable density.

## States

Trigger chip:

| State | Visual | Tokens |
|---|---|---|
| default | Mark, model, effort, chevron | `text.primary`, `text.secondary`, `text.tertiary` |
| hover | Fill | `fill.hover.raised` |
| pressed | Fill | `fill.pressed.raised` |
| focus-visible | Ring | `border.focus`, 2 px, offset 2 px |
| open | Fill, chevron-up | `fill.selected.raised` |
| disabled | While a session is starting or the agent cannot change model: label and mark | `text.disabled`; tooltip gives the reason |
| loading | The model list is loading: label shows "Loading models" with a 12 px spinner after 200 ms | `agent.running` |
| error | Agent signed out or missing: warning glyph before the name, tooltip "Sign in to Claude" | `status.warning.fg` |
| long content | Model name truncates with an ellipsis, tooltip shows the full name and effort | `text.primary` |

Popover contents:

| State | Visual | Tokens |
|---|---|---|
| closed / open | See anatomy | |
| searching | The list shows matches from every agent in one list (provider icon identifies the agent), matched letters in `body-strong`; the rail shows no active bar. Reasoning column keeps the current model | `text.primary` |
| no match | "No models match "xyz"" and a "Clear search" text button, centred in the list | `text.secondary`, `accent.fg` |
| Favourites empty | "No favourites yet" and "Star a model to keep it here" | `text.secondary`, `text.tertiary` |
| model row hover / selected | See anatomy | `fill.hover.overlay`, `fill.selected.overlay` |
| model without reasoning levels | Reasoning column shows "This model has no reasoning levels" and no slider (also for one level, A98) | `text.tertiary` |
| Fast unavailable | Fast segment disabled, tooltip "This model has no Fast mode" | `text.disabled` |
| loading | Skeleton rows (6) on the list; the reasoning column shows an empty 28 px track | `fill.skeleton` |
| error | Per agent: "Could not load models from Claude" with Retry in place of the list | `status.danger.fg` |
| signed out | "Sign in to use Claude models" with a "Sign in" button that starts the account flow (X-AC-01) | `text.secondary` |
| disconnected host | The lists show the last known models; a one-line note at the top of the model column: "Last known models, 5 min ago" with the unreachable glyph. Speed can still be chosen; it applies at the next message | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Selection model.** Clicking a model row selects it: the chip updates, the reasoning column shows that model's levels and keeps the previous effort when the new model has it, otherwise the model's default. The popover stays open so the effort can be set. It closes only on Escape or an outside click. Enter on a model row selects the model and keeps the popover open. Changing the level on the reasoning slider (drag, a click on the bar or on a level label, or the keys) keeps it open. Standard or Fast applies immediately; the popover stays open for it.
- **Shortcuts `Mod+1` to `Mod+4`.** They exist only while the popover is open and select the first four rows of the current view (the Favourites view lists favourites in order; an agent view lists its featured models). They select exactly as a click does. Outside the popover the keys keep their normal meaning. Extensions cannot claim them.
- **Open.** The chip opens on Enter, Space, Down arrow or click. A configurable shortcut (default `Mod+Shift+M`, rebindable in D-ST-04) opens it from the Composer.
- **Keyboard inside.** Focus lands in the search field. Tab moves search, model list, reasoning column, speed control; Shift+Tab goes back. Arrows move inside a column (roving focus); Left from the model list goes to the agent rail and Right goes to the reasoning column. Typing any printable character while the list has focus moves it to the search field. `Alt+Enter` toggles the focused row as a favourite (also in the row's context menu). Escape closes and returns focus to the chip. The popover is not modal; it traps no focus beyond Tab order but closes when focus leaves it.
- **Favourites.** The star on a row toggles it. Favourites appear in the Favourites view of the rail, in the order the user starred them; the first four are the `Mod+1` to `Mod+4` targets there.
- **Search.** Filters as you type across all agents, case-insensitive, matching name and provider. Enter selects the first match.
- **Pointer.** Hover moves the highlight; the star fades in over `duration.fast`. Moving across rail buttons does not change the list until a click.
- **Resizing.** Width 560 px; the list scrolls inside a fixed maximum height. The popover repositions on window resize.
- **Motion.** The popover fades in with a 4 px move toward its final position over `duration.base` with `ease.enter`; exit uses `ease.exit`. Hover and selection fills step over `duration.fast`. Reduced motion: instant.
- **Accessibility.** Trigger is a button with `aria-haspopup="dialog"` and `aria-expanded`; name "Model: Opus 5.5, Medium". The popover is a non-modal dialog named "Choose model". The rail is a vertical tab list named "Agents"; the model column is a listbox named "Models" (current model `aria-selected`); the reasoning column is a radio group named "Reasoning for Opus 5.5"; the speed control is a radio group named "Speed". Changes are announced politely: "Opus 5.5, Medium selected". The shortcut badges are included in each option's description.
- **Extension limits** (contribution point 12). An extension adds an agent or provider (icon 20 px, name), models (name up to 40 characters, provider icon, level names up to 16 characters, speed modes, access modes) and may request a usage ring slot. Its icon is drawn in the token colour for its state and its name is in the tooltip. It cannot take `Mod+1` to `Mod+4`, change the layout, become the default without the user's consent, or reorder core agents.

## GPUI notes

- Draw the popover as a deferred, anchored element above the Canvas (`z.popover`) with an occluding hit area; compute placement from the trigger bounds and flip when needed.
- Use fixed 32 px rows so the model list can be a `uniform_list` (virtualised past about 100 models). Columns are flex children with `overflow_y_scroll`.
- Define one key context ("ModelPicker") with actions for select model by index, toggle favourite, next and previous column. `Mod+1` to `Mod+4` bind only in this context.
- The search field is the shared text input. Keep the focus handle on the popover root and one handle per column for roving focus.
- No blur and no gradient behind it; the floor is `shadow.e2` and the border.

## React Native notes (Android)

- Bottom sheet (`layout.sheet.max-height` 90 percent), title "Model", drag handle, `radius.2xl` top corners. One column: a horizontal row of agent chips (48 dp hit), a search field, model rows (56 dp, provider icon, name, check on the current one, star as a 48 dp target), a "Reasoning" radio list (48 dp rows) and a Standard | Fast segmented control. Access is the Composer's access lock, not part of this sheet.
- No shortcut badges and no hover star: the star is always visible. The trigger chip is 44 dp high with a 48 dp hit target.
- TalkBack: sheet title announced on open; each row states selected, favourite, and its reasoning levels count.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Container | Anchored popover, three columns | Bottom sheet, one column |
| Shortcuts | `Mod+1` to `Mod+4`, `Alt+Enter` for favourite | None |
| Star | Visible on hover and focus | Always visible |
| Chip height | 24 px | 44 dp |

## Copy

- Placeholder: "Search models". Rail tooltips: "Favourites", the agent name. Header: "Reasoning". Footer: "Standard", "Fast".
- Empty or error: "No favourites yet", "Star a model to keep it here", "No models match "xyz"", "Could not load models from Claude", "Sign in to use Claude models", "This model has no reasoning levels", "This model has no Fast mode".
- Disconnected: "Last known models, 5 min ago".

## Token gaps and open points

Token gaps: none. Widths and heights are `layout.model-picker.*` tokens; the row height follows density: `size.control.default` (28) or `size.control.comfortable` (32), and the shortcut key cap uses `size.kbd.height` (16 px, not a separate 20 px size; see 6.5, X10).

Open points:
- Resolved (consistency pass, 2026-11-13): access mode is the Composer's own access chip (desktop) or lock (Android), as in the approved v47 and v50 composers (A33 over A27); the picker has no access control.
- `Mod+1` to `Mod+4` select by position in the current view (resolved in 6.5, X9).
- Provider marks: resolved by A16 and A18: full-colour agent icons everywhere, the generic agent mark only for an agent with no icon; previews use the agent icon symbols.
