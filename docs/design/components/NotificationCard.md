# Notification card

## Purpose

A persistent card that tells the user something needs their attention and offers the one or two actions that deal with it: an agent is waiting or failed, an app update is ready, an account needs to sign in. Cards stay until the user acts or dismisses them. Short, passing results are Toasts.

Inventory IDs: D-NT-01 (notification card stack), X-UP-01 (app update notice, shown as a card on desktop and on the phone's home), X-AC-01 (account needs sign-in). The OS notification (X-NT-02) only announces attention; acting happens on the card.

## Anatomy

- Card (desktop, floating): `surface.overlay`, `border.default`, `radius.lg`, `shadow.e2`, width 360 px (minimum 320, maximum 400; the same widths as Toast), padding `space.3`.
- Kind plate: 28 px, `radius.md`, with a 16 px glyph. The kind is always shown as a word in the meta line.

| Kind | Plate fill and glyph | Meta word | Example |
|---|---|---|---|
| Attention | `agent.waiting.bg.overlay`, `agent.waiting` circle with question mark | "Waiting for you" | "oppm: Run command? pnpm install" |
| Error | `agent.error.bg.overlay`, `agent.error` triangle | "Error" | "NexusCare: the run failed" |
| Update | `status.info.bg`, `status.info.fg` download icon | "Update" | "MassOrca 1.4.2 is ready" |
| Sign-in | `status.warning.bg`, `status.warning.fg` user icon | "Sign-in needed" | "Claude was signed out" |
| Info | `surface.sunken`, `text.secondary` info icon | "Note" | An extension's card (name-prefixed) |

- Content: title in `body-strong` (up to 60 characters, one line), body in `body-sm` `text.secondary` (up to 160 characters, two lines), meta line in `caption` `text.tertiary`: source (session, project or extension), the kind word and the age ("2 min ago", tabular).
- Actions: at most two, right-aligned at `size.control.compact`: primary (`accent.default`) and a ghost secondary. The sign-in card has exactly one action ("Sign in"; ask once, in place, with one action).
- Dismiss: x icon button at the top right, `size.control.compact`, tooltip "Dismiss".
- Update card body by state: Available ("MassOrca 1.4.2 is available", "Download" and "Later"); Checking (spinner and "Checking for updates"); Downloading (Progress bar 4 px with "Downloading, 64%"); Ready ("Restart to finish the update", "Restart now" and "Later"); Error (octagon-alert, "Update failed: <reason>", "Retry").
- Stack: cards sit in the bottom-right corner of the Canvas, `space.4` from the right edge and from the Status bar, `space.2` apart. At most 3 are visible, newest first within a priority; a fourth and later are folded into a "+2 more" row that expands the stack (maximum height 60 percent of the Canvas, then it scrolls). Order by priority: Attention and Error, Sign-in, Update, Info. Cards occupy the lowest slots; Toasts stack above them, so a card never moves when a toast comes or goes.

## Variants and sizes

| Variant | Difference |
|---|---|
| Floating (desktop) | As above, over the Canvas. |
| Inline (Android) | No shadow, `surface.raised` with `border.default`, full width minus `space.4` gutters, at the top of the home screen (M-HM-01) and in the session. Actions 48 dp. |
| Folded row | "+2 more", one line, `surface.overlay`. |

## States

| State | Visual | Tokens |
|---|---|---|
| default | As above | `surface.overlay`, `text.primary`, `text.secondary` |
| hover | Card fill | `fill.hover.overlay` |
| pressed | Card or button pressed | `fill.pressed.overlay`, `accent.pressed` |
| focus-visible | Ring on the focused control; the card itself is a group, not a button | `border.focus`, 2 px, offset 2 px |
| selected | Not applicable | not applicable |
| disabled | Actions disabled while the host is unreachable or the action is running | `fill.disabled`, `text.disabled` |
| loading | Update checking and downloading as above | `agent.running`, `accent.default` |
| empty | The stack is not drawn when there are no cards | not applicable |
| error | Update failed and Error kind as above | `agent.error`, `status.danger.fg` |
| entering | Slides in 8 px and fades in over `duration.moderate`, `ease.enter`; cards below the new one move over `duration.base`, `ease.standard` | motion tokens |
| leaving | Fades out over `duration.base`, `ease.exit`; the stack closes the gap | motion tokens |
| long content | Title one line with ellipsis, body two lines with ellipsis, meta truncates the source first; full text in the tooltip | `text.primary` |
| disconnected host | A card about a session on an unreachable host reads "Last known: waiting for you, 5 min ago" with the unreachable glyph; the plate takes `agent.unreachable.bg.overlay`; "Open" still works, answer actions are disabled. Never "stopped" | `agent.unreachable`, `agent.unreachable.bg.overlay`, `text.tertiary` |

## Behaviour (handoff)

- **Lifecycle.** A card is created when a session needs the user (approval, question, secret request), a run fails, an update changes state, or an account expires. It resolves itself when the cause is gone (the user answered in the chat, the account signed in) and leaves with the exit animation. The user can dismiss it; dismissal hides the card, not the state, and the sidebar row and chat keep showing it. One card per session per kind: a repeat updates the existing card.
- **Do not disturb.** Cards follow notification settings (D-ST-08) and the OS do-not-disturb; extensions cannot bypass either.
- **Keyboard.** The stack is not a region in the F6 cycle. `Mod+Alt+N` (command "Focus notifications", also in the palette) moves focus to the first card; Up and Down move between cards; Tab moves within a card (actions, then dismiss); Escape dismisses the focused card and returns focus to where it was; Delete and Backspace do not dismiss. Cards never take focus on their own.
- **Pointer.** Click "Open" or the card's primary action. Hover pauses nothing (cards do not time out).
- **Resizing.** Width fixed at 360 px; in a Conversation narrower than 400 px the stack uses the Canvas width minus `space.4`.
- **Motion.** As in States. Reduced motion: instant.
- **Accessibility.** The stack is `role="region"` named "Notifications". A new card is announced once, politely, as "<kind word>: <title>". Error cards are announced assertively (`role="alert"`). The dismiss button is named "Dismiss <title>". Progress in the update card is a `progressbar`.
- **Extension limits** (contribution point 14). Kind info, success, warning or danger; a card has a title (up to 60 characters), a body (up to 160) and up to 2 actions. The extension's name always prefixes the text ("Acme: ..."). It cannot take focus, play a sound, bypass settings or do-not-disturb, imitate core messages, or send OS notifications without the per-extension permission (X-NT-02).

## GPUI notes

- Draw the stack as one deferred layer anchored to the Canvas bottom-right. Each card is a normal element; height changes (entering, leaving) animate a per-card offset.
- The shadow is the token box shadow; no blur behind. Update progress is a quad.
- Keep the card list in the notification service so it survives window changes; the view renders from it.

## React Native notes (Android)

- Cards are in-flow components, not floating. The home screen shows them above the host list; a session shows attention cards above the Composer. Full width, `space.4` gutters, actions 48 dp with 8 dp between, swipe to dismiss plus a visible dismiss button. System notifications handle background attention (M-NT-01 sets what is enabled). TalkBack announces a new card politely and reads its actions.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Placement | Floating stack, bottom right | Inline at the top of home and in the session |
| Elevation | `shadow.e2` | None (`surface.raised` and border) |
| Dismiss | x button, Delete key | x button, swipe |
| Background attention | OS notification | System notification |

## Copy

- Kind words: "Waiting for you", "Error", "Update", "Sign-in needed", "Note".
- Actions: "Open", "Dismiss", "Download", "Later", "Restart now", "Retry", "Sign in".
- Answers on an approval card: "Approve" and "Deny", the words of the chat's approval card (X-CH-02).
- Update: "MassOrca 1.4.2 is available", "Checking for updates", "Downloading, 64%", "Restart to finish the update", "Update failed: <reason>".
- Account: "Claude was signed out".
- Host: "Last known: waiting for you, 5 min ago".
- Folded row: "+2 more".

## Token gaps and open points

Token gaps: none. Unreachable cards take `agent.unreachable.bg.overlay` on the plate.

Open points:
- The placement of cards and toasts in one corner (cards lowest, toasts above) is a decision here; the design system names both "bottom right of the Canvas".
- Whether dismissed Attention cards should come back after a time if the session still waits; not designed.
- Priority order of kinds is proposed.
