# 0091: Project icons

Status: accepted, 2026-11-24.

## Context

WS-01, adding an existing git repository as a project, registered the repository at its top level, refused a duplicate
path or name and kept the name, but a project had no icon. pv96 shows a repository icon in its sidebar. It stores one of
a few kinds, picks it from the repository's files when the repository is added, and lets the owner replace it. This
decision copies that shape and those choice rules. It does not copy pv96's storage or its network sources.

## Decision

- A project has an optional `icon`, one of three kinds:
  - `lucide`: a Lucide icon name, letters and digits, starting with a letter, at most 40 characters.
  - `emoji`: one emoji, at most 16 UTF-16 units after trimming.
  - `image`: a base64 PNG data URI, at most 256 KiB of image and 400 KiB as a data URI. A `file` image may also be a WebP.
  - An image has a `source`: `upload` (a PNG the owner chose) or `file` (detected in the repository), and an optional
    `label` of at most 80 characters. Text is trimmed before it is checked.
- The icon is stored as its JSON in the nullable column `projects.icon`, appended as a migration step. `NULL` is the
  default icon. No existing row changes.
- `project.add` takes an optional `icon`. An icon given there is checked and kept. Without one, the service detects an icon
  from the project's own files, for a folder project too:
  1. the first conventional image among 15 locations (`favicon`, `public/favicon`, … `icon`), each tried as `.png`, then
     `.webp`;
  2. else the `<link rel="icon">` or `shortcut icon` of `index.html`, `public/index.html` or `src/index.html`. Its href must
     have no query and must resolve inside the project, relative to its own page, then under `public/`, then at the root.
  Every read is bounded: a file over 256 KiB, or an entry page over 256 KiB, is not read. Nothing is spawned.
- `project.update` (capability `project-update/1`) changes the icon. An absent `icon` keeps it, `null` clears it so the
  project shows the default, and an icon sets it after the same checks as `project.add`. A refused icon leaves the stored
  one as it was. It publishes `project.changed`.
- The CLI takes `--icon emoji:<emoji>`, `--icon lucide:<name>` or `--icon image:<PNG file>` on `project add`, and
  `project update <PROJECT>` takes `--icon` or `--clear-icon`. The CLI reads the image file, within the size limit, and sends
  it as an upload. `project list` shows an `ICON` column, and `project show` and `project update` print the icon.
- The desktop sidebar shows an emoji icon before the project's name. It does not draw lucide or image icons yet, because
  the shared sidebar row takes only the built-in icon set.

## Not copied from pv96

- The `homepage` favicon from `package.json`. It is a URL on a third-party favicon service, so drawing it sends the homepage's
  domain there. Adding it needs that decision first.
- The GitHub owner avatar. It needs the git remote, which means a spawned process and a network request.
- Icons declared in JSX route files such as `app/root.tsx`.
- pv96's reset that re-reads the GitHub avatar. Here `project.update` with `null` is the only reset.

## Consequences

- The new fields are optional and left out when unset, so a client that does not know them still reads projects. A client
  must check that `project-update/1` is offered before it calls `project.update`.
- An image icon adds up to about 350 KiB of base64 to its project row, and `project list --json` prints the whole data URI.
- Lucide and image icons are stored and listed, but the desktop does not draw them yet.
