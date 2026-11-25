# Extension manager row

## Purpose

One installed extension as a row of Marketplace > Installed: what it is, its state, and the two controls that act on it (the Enabled switch and More). The Installed list is one group card of rows. Discover, Updates and the categories keep the extension card (ExtensionCard.md).

Inventory ID: D-EX-03 (extension manager). Screen spec: teams-extensions-settings.md 7.4, artboard `bt-extension-manager.dc.html`. A68 puts every installed extension in Marketplace > Installed. Desktop only.

## Anatomy

- Group: one card (`surface.raised`, `border.default`, `radius.lg`) holding the rows, a list named "Installed extensions". Rows are separated by a 1 px `border.subtle` line.
- Row: `space.3` above and below, `space.4` at the sides, `space.3` between parts. 56 to 64 px high.
- Plate: 36 px square, `surface.sunken`, `radius.md`. The extension's icon is drawn at 16 px in `text.secondary`. An agent draws its own mark (A16) at the small size.
- Name: `body-strong`, one line. It is a link to the item page, with a focus ring and a Tab stop. It is `text.secondary` while the extension is disabled.
- Byline: `body-sm` in `text.secondary`: publisher, version and what it adds, joined by " · ". It truncates with an ellipsis.
- Status column: 200 px. The word comes from the desktop model; the row draws the glyph or badge for its state.
- Enabled switch: the shared Switch with no visible label, named "Enable <name>". On for an enabled extension, off for a disabled or failed one, disabled while it installs.
- More: a ghost icon button named "More actions for <name>".

## Variants

| Status word | Status column | Switch |
|---|---|---|
| Enabled | success glyph (circle-check) and the word, in `status.success.fg` | on |
| Disabled | neutral Badge "Disabled" | off |
| Update available 2.1.0 | info Badge with the word | on |
| Installing, 40% | the word in `text.secondary` | disabled |
| Failed to start | danger glyph (octagon-alert) and the word in `status.danger.fg` | off, still usable |

The row has no Update button. Update stays on the Updates page and the cards.

## States

| State | Visual | Tokens |
|---|---|---|
| default | As anatomy | `surface.raised`, `border.default` |
| focus-visible | Ring on the name link, the switch and the More button, not on the row | `border.focus`, 2 px, offset 2 px |
| installing | Switch disabled; the word shows the percent | disabled switch tokens |
| error | Failed to start | `status.danger.fg` |

## Behaviour (handoff)

- Name press: opens the item page. Switch press: sends the change to the caller, which applies it; the switch settles when the service confirms.
- More press: opens the row's menu at the pointer, which lists Remove only. Remove, in the danger colour, opens the uninstall confirm that the item page's Danger zone opens. Roll back and View log join the menu when extensions.rollback and extensions.log exist. Up and Down move in the menu, Enter chooses and Escape closes it.
- A failed row has no link of its own. The item page shows the state, not the failure, and the name already opens it.
- Keyboard: each row is a list item. Tab visits the name, the switch and More.
- Accessibility: rows are list items named by the extension's name. The switch is named "Enable <name>". The status word is plain text, read in order.

## Copy

- Status words, from the desktop model: "Enabled", "Disabled", "Update available <version>", "Installing, <percent>%", "Failed to start".
- Accessible names: "Enable <name>", "More actions for <name>".

## Token and spec notes

- The 36 px plate and the 200 px status column are the artboard's measures. No size token covers them, so they are constants of the component.
- The row has no Settings button, because the artboard rows have none. The card keeps Settings in Discover.
