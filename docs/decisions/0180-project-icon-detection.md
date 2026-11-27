# 0180: Project icon detection sources

Status: accepted, 2026-11-26.

## Context

Decision 0091 detects a project's icon from its files when the project is added without one: the conventional PNG or
WebP locations, then the icon an HTML entry page declares. pv96 also reads the icon a JSX route root declares. This
decision adds that source, and the `icon` field of `package.json`, which pv96 does not read (it reads the homepage
favicon, which 0091 leaves out). Every icon read stays inside the repository. It extends 0091 and drops its note that
JSX route roots are not copied.

## Decision

- Declared icons are read from these files, in this order: `index.html`, `public/index.html`, the JSX route roots
  `app/routes/__root.tsx`, `src/routes/__root.tsx`, `app/root.tsx`, `src/root.tsx`, then `src/index.html`. A file declares
  its icon with the first `<link rel="icon">` or `shortcut icon` tag, else with the first `{ rel: "icon", href: "…" }`
  object, the form of a route's `links`. The href rules of 0091 apply: no query, and it must resolve inside the repository.
  Nothing is evaluated or rendered. A computed href such as `href={…}` is not followed, and an inline JSX SVG logo gives
  no icon.
- Only when no file gives an icon, the `icon` field of `package.json` is used. It is either a PNG or WebP path inside the
  repository (a leading `/` means the repository root), or a PNG or WebP data URI. A URL is never followed, and the file
  is read up to 128 KiB, as pv96 reads it.
- Every icon read, including `package.json`, must resolve to a file inside the repository, through symbolic links too.

Still not copied: the homepage favicon of `package.json` (a third-party favicon service, see 0091), SVG logos, and the
GitHub owner avatar. The avatar needs an HTTP client, and the service crates have none; adding one is a separate decision.

## Consequences

- An existing project changes its icon only where a file that was not read before now declares one: a JSX route root, or
  an object-form `links` entry in an HTML page. A project with no file icon may now take its `package.json` icon.
- The containment check changes existing reads too: an icon file reached through a symbolic link that leaves the
  repository is no longer read. The test for it runs on Unix only, because creating symbolic links on Windows needs a
  privilege that not every host grants.
