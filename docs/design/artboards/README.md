# Design artboards

These are the HTML artboards of the MassOrca screens and components: the desktop app (`sh-*` shell and hosts, `ws-*` workspaces and sessions, `bt-*` Bot Teams, extensions and settings) and the Android app (`an-*`). `Main.dc.html` is the index page. `massorca.css` holds the design tokens the artboards use, and `massorca-logo.svg` is the logo.

## Version

Snapshot of the design project taken on 2026-11-20, the date of its latest artboard edits. Only the current version of each artboard is kept: one file per artboard on the design canvas, plus the assets those files reference. Version history, editor state and the canvas layout file were left out. Sample data was scrubbed of real names, hosts and addresses.

## Source of truth

The artboards settle spec conflicts:

1. An artboard wins over any document.
2. If the artboard cannot settle it, the component document wins over a screen note.
3. Between screens, the later revision wins.

## Opening them

Open any `.dc.html` file in a browser, straight from this folder. They work offline: styles and images resolve by relative path or are embedded.

- Each file also loads `./support.js`, the design tool's runtime, which is not part of this folder. The request fails harmlessly and the static markup renders on its own. Artboards with a theme switch then show their default theme.
- Fonts (Geist, JetBrains Mono) come from Google Fonts. Offline, the browser falls back to system fonts.
