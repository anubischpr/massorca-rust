# Permission review

## Purpose

The one screen that shows what an extension asks for before it is installed, updated or rolled back, with a plain-words risk notice, and then runs the install. The user grants permissions here and nowhere else (A12). It is security chrome: the core draws it with validated tokens and no extension can put content in it.

Inventory ID: D-EX-02 (install and permission review; absorbs the plugin and skill install, consent and rollback dialogs). Entered from the ExtensionCard "Review and install", "Update" and "Roll back". Removal uses the confirm dialog (X-DLG-01).

## Anatomy

- Container: a Dialog (`surface.overlay`, `border.default`, `radius.xl`, `shadow.e3`, `scrim`), width `layout.dialog.width.lg` (560 px), maximum height 90 percent of the window, padding `space.4`. Header and footer are pinned; the body scrolls.
- Header: extension icon plate 40 px (`size.avatar.lg`), name in `title`, publisher, source and version in `body-sm` `text.secondary` and `mono-sm` for the version, and a close icon button. In update and rollback modes a line "1.4.0 to 1.5.0" (or "1.5.0 back to 1.4.0") in `label`.
- Risk notice (Banner at the top of the body): a plain sentence that sums up the worst thing the extension can do, with glyph and word.
  - Low: info glyph, "Note", `status.info.bg`: "This extension can only add items to the interface."
  - Medium: triangle-alert, "Check", `status.warning.bg`: "This extension can read files in your project and use the network."
  - High: octagon-alert, "High risk", `status.danger.bg`: "This extension can run commands on your computer."
  The level is computed by the core from the permission set; the manifest cannot set it.
- "This extension asks for" list: one row per permission kind, separated by `border.subtle`. Row: icon 20 px (`text.secondary`), title in `body-strong`, scope in `mono-sm` (domains, folders, command patterns; three shown, then "+4 more" that expands), the extension's own reason in `body-sm` `text.secondary` (plain text up to 120 characters, labelled "Reason from the extension"), and a risk marker on the right: glyph, word and colour ("Low", "Medium", "High").
  - Network access (globe): domains. "Any site" is High.
  - Files (folder): read or write, folders. Whole disk or home folder is High.
  - Commands (terminal): programs it may run. Any command is High.
  - Notifications (bell): "Show notifications". Low.
- "What it adds" list: the contribution points it uses, as chips with icons ("Rail item", "2 commands", "Settings section", "Theme"). This part is information only.
- Install target (only when more than one host): a Select "Install on", default this computer.
- Acknowledgement (High risk only): a Checkbox "I understand this extension can run commands on this computer." The install button stays disabled until it is checked.
- Footer (top `border.subtle`): "Cancel" (secondary) and the primary action: "Allow and install", "Allow and update" or "Roll back". For High risk the primary stays `accent.default`; it is not a destructive action.
- Progress area (replaces the body during install): a list of steps "Downloading", "Checking the package", "Installing" with Progress (bar), each step with a state glyph.

## Variants and sizes

| Mode | Differences |
|---|---|
| Install | Full list, risk notice, primary "Allow and install". |
| Update | Only new or changed permissions are listed first under "New in 1.5.0", each with a "New" chip (`accent.subtle.overlay`, `accent.fg`); the unchanged ones are collapsed under "Unchanged (3)". If nothing new is requested the screen says "No new permissions" and the primary is "Update". |
| Rollback | Shows the target version and the permissions that return to the older set; primary "Roll back". |
| Installing | Progress area, only "Cancel" before the package check ends. |
| Android | Not designed (extensions are desktop only). |

## States

| State | Visual | Tokens |
|---|---|---|
| reviewing | As anatomy; the primary is disabled for High risk until the acknowledgement is checked | `fill.disabled` for the disabled primary |
| hover | Buttons and rows' expanders: their own | `accent.hover`, `fill.hover.overlay` |
| pressed | Buttons | `accent.pressed`, `fill.pressed.overlay` |
| focus-visible | Ring 2 px, offset 2 px; initial focus is the dialog title, not a button | `border.focus` |
| selected | A checked acknowledgement | `accent.default` |
| disabled | Install while the target host is unreachable or the extension is incompatible | `fill.disabled`, `text.disabled` |
| installing | Progress steps; the dialog cannot be dismissed with Escape or the scrim during the checking step | `agent.running`, `accent.default` |
| installed | The body is replaced by a success line with `circle-check` in `status.success.fg`, "Installed", and the buttons "Open settings" (when it has a section) and "Done" | `status.success.fg` |
| failed | Banner `status.danger.bg`: "Not installed: <reason>. Nothing was changed." with Retry and "Copy details" | `status.danger.fg` |
| rollback | As Variants. After success "Rolled back to 1.4.0" | `status.success.fg` |
| loading | Skeleton rows while the permission list is fetched | `fill.skeleton` |
| empty | An extension that asks for nothing: "This extension does not ask for any permission." The risk notice is Low | `text.secondary` |
| error | Fetch failure: ErrorState "Could not read the permissions" with Retry | `status.danger.fg` |
| long content | Scope lists collapse to three items with "+N more"; the reason text truncates at 120 characters; the body scrolls | `text.secondary` |
| disconnected host | The target host is unreachable: a Banner "Can't reach VPS. Nothing can be installed there until it is back." with the unreachable glyph; the primary is disabled; the user may switch the target | `agent.unreachable` |

## Behaviour (handoff)

- **Keyboard.** The dialog traps focus. Initial focus is the title (a focusable heading), so Enter does nothing until a button is focused. Tab order: expanders, Install target, acknowledgement, Cancel, primary. Escape cancels (except while checking the package). Enter on the focused primary installs only when enabled.
- **Focus return.** Closing returns focus to the card that opened it; after a successful install, focus goes to that card's Switch.
- **Pointer.** Click an expander to show the full scope list. Text is selectable (domains, paths) so users can copy it.
- **Resizing.** Fixed `layout.dialog.width.lg` (560 px), or the window width minus `space.4` when narrower. Header and footer pinned.
- **Motion.** The dialog enters with opacity and scale 0.98 to 1 over `duration.moderate`, `ease.enter`; the scrim fades with it. Progress bars fill without easing. Reduced motion: instant.
- **Honest state.** Nothing says "installed" before the host confirms it and records the version for rollback. A failed install leaves the previous state unchanged and says so.
- **Accessibility.** `role="dialog"` with `aria-modal`, named "Install Acme Metrics" (or "Update", "Roll back"). The risk notice is the dialog's description and is announced when it opens, politely. Each permission row reads "Network access, Medium risk, api.acme.dev, Reason from the extension: <text>". Progress steps are a live region; completion is announced.
- **Extension limits.** The manifest supplies plain text only: name, publisher, description, permission reasons, scopes and the contribution list, truncated to limits. It cannot supply Markdown, links, colours, icons beyond its own, a risk level, a pre-checked box, or any element in this dialog. The dialog's tokens and layout are fixed by core and validated with every theme.

## GPUI notes

- Use the shared Dialog layer. The body is a scroll container with sticky header and footer rows.
- The risk level and the permission rows are produced by the core from the parsed manifest; the view never trusts manifest-supplied styling.
- Make the title a focusable element and set it as the initial focus; keep the primary button's `disabled` tied to the acknowledgement state and the host reachability.
- The install runs in the core; the view subscribes to its step events.

## React Native notes (Android)

Not designed. Extensions are not installed from the phone in v1.

## Desktop vs Android differences

Desktop only.

## Copy

- Titles: "Install Acme Metrics", "Update Acme Metrics", "Roll back Acme Metrics".
- Risk: "Note: This extension can only add items to the interface.", "Check: This extension can read files in your project and use the network.", "High risk: This extension can run commands on your computer."
- Rows: "Network access", "Files", "Commands", "Notifications", "Reason from the extension", "+4 more".
- Update: "New in 1.5.0", "Unchanged (3)", "No new permissions".
- Acknowledgement: "I understand this extension can run commands on this computer."
- Buttons: "Cancel", "Allow and install", "Allow and update", "Roll back", "Open settings", "Done", "Retry", "Copy details".
- Messages: "Not installed: <reason>. Nothing was changed.", "Installed", "Can't reach VPS. Nothing can be installed there until it is back."

## Token gaps and open points

Token gaps: none. The width is `layout.dialog.width.lg`.

Open points:
- How the core computes the risk level (rules per permission kind) is proposed here, not decided; it needs the extension model decision.
- The acknowledgement checkbox for High risk is a proposal; the inventory only asks for "a risk notice".
- The list of permission kinds beyond network, files, commands and notifications (vault references, browser, clipboard) is not final.
- Whether the app may show a signature or verified-publisher state; none exists yet.
