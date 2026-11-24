# 0061: Extension package format

Status: accepted, 2026-11-22. Design: `docs/architecture/extensions.md` §2 to §6. Code: `crates/ext-package`
(`massorca-ext-package`), pure and without I/O beyond the bytes its callers pass in.

The format is complete in this PR and its base (`ext/package-format`): the manifest, the contribution kinds, the permission
vocabulary, identifiers, paths, the listing, risk and update review, uninstall effects and the archive check.

## Context

0001 fixes the extension model: a `publisher.name` package, a closed list of contribution kinds, a closed permission
vocabulary, risk computed by the core, and a SHA-256-pinned archive. The Marketplace item page (owner direction) must
render any item from data, with no item-specific code. Agent CLIs are process contributions (0050), and the agents
session owns decision 0059 (in progress), which defines what an agent declaration contains. The crate that checks the
package has to exist before the store, the lockfile and the Marketplace do.

## Decision

### Manifest

- `extension.json` is parsed with serde, with unknown fields rejected at every level. An unknown contribution kind, an
  unknown permission kind or an unknown scope field is an error, never ignored.
- A JSON shape error (wrong type, unknown field, unknown kind, malformed `api` or `version`) stops at the first one,
  reported with its JSON-pointer path. Every semantic rule is then checked in one pass, and all problems are returned
  together as a list of `{path, message}`.
- `id` is `publisher.name`: exactly one dot, each side lowercase `[a-z0-9_-]`, at most 32 characters, not starting
  with `-`. The `publisher` field must equal the prefix. The rule is a subset of the server's owner-name rule, so an id
  also works as `ext:<id>`.
- `api` is `major.minor` (the WIT world). `version` is semver. `requires.massorca` is a semver requirement.
  `views` is the view-schema major, starting at 1, and is required when views are contributed.
- `surfaces` (desktop, android) and `modes` are non-empty and unique. Mode names follow the contribution id rule.
- `main` is a package path ending in `.wasm` or `.js`. Without `main` the package is data-only: no handler, and no
  logic-only kind (`events`, `tools`, `itemSources`, `jobs`, `importers`).
- Every file reference is a package-relative path: no absolute path, no `..`, no `.`, no backslash, no colon (so no drive
  letter), no empty segment, no control character. `check_files_present` checks the references against a file list.

### Contribution kinds

Seventeen kinds, typed, in three tiers. §3 lists `themes` and `iconThemes` in one row, which is why the count in the
brief (16) differs. Ids are unique within their kind (setting keys within `settings`).

- **Data** (`manifest::data`, `settings`, `views`): `commands` (a handler is optional and needs a `main`), `keybindings`,
  `themes`, `iconThemes`, `locales`, `recipes`, `agentRules` (documents), `skills` (folders that hold a `SKILL.md`),
  `settings` and `views`.
  - A setting declares `type` (boolean, integer, number, string), a `default` that must match the type, and an optional
    `section`: `basic` (default) or `advanced`.
  - A view's `slot` is one of the fourteen slots of §7.
- **Logic** (`manifest::logic`): `importers`, `events`, `tools`, `itemSources`, `jobs`. Each needs a `main`. Event names
  are dotted lowercase. A tool's `input` is a schema file that must be in the package.
- **Process** (`manifest::agent`): `agentAdapters` and `mcpServers`. Each needs a `process` permission. See below.

### Agent packages

- `agentAdapters` entries are `{ id, kind, cli? }`. `id` follows the kind-id rule (lowercase `[a-z0-9-]`, 1 to 32
  characters). `kind` must be exactly `agents/<id>.json`. The crate never parses the declaration (core-agents does).
- The ids `claude`, `codex`, `antigravity` and `gentle-shell` are reserved to publisher `massorca`. Another publisher
  using one is a validation error.
- An agent adapter or an MCP server requires a `process` permission.
- The `cli` block declares how the vendor CLI is obtained. The package bundles no binary:
  - `binary`: a bare program name, with no path separator, drive or whitespace.
  - `verify`: a non-empty argv whose first item equals `binary`.
  - `source`: an https URL to the vendor's official install documentation.
  - `install` and `uninstall`: lists of `{ os, command }`. `os` is `windows`, `linux` or `macos`, with at most one entry
    per OS in each list, and each command is a non-empty argv. `uninstall` is optional.
- **Install rule.** Installing registers an already-present CLI instantly. Otherwise the review shows the exact official
  command, and it runs only after the user confirms, as a High-risk step. It never runs silently, and never elevated
  unless the vendor's own installer asks and the user accepts there. Removing the package unregisters the agent. It
  offers to remove the CLI only if the package installed it; the lockfile records that later, in `core-extensions`, not
  in this crate. A headless server uses the same flow through `massorca extension install ... --yes-run-installer`, an
  explicit flag whose CLI lands in a later PR.
- **Alternatives for the install rule.** Bundling vendor binaries was rejected: licensing, update cadence and size. Giving
  instructions only was rejected: the owner directed a usable CLI after install.

### Listing (Marketplace page data)

The optional top-level `listing` object carries what the item page shows. Unknown fields are rejected. `icon` is
replaced by `listing.logo`.

- `logo`: a package path to an `.svg` or `.png`.
- `category`: `extensions`, `agents`, `skills`, `services` or `themes`. When absent it is derived (see below).
- `developer`: `{ name, url? }`. `url` must be https.
- `website`, `privacy`, `terms`: optional links to the publisher's website, privacy policy and terms of service. Each
  must be https, as `developer.url` is. The item page's Information part shows each one the package names.
- `mention`: no `@`, no whitespace, at most 32 characters.
- `banner`: `{ image?, examples }`, with one to six examples. A `mention` example's `text` must equal `@` + `mention`.
  A `command` example's `text` must equal a declared command's `keys` or `id`, compared literally. A `usage` example is a
  caption only. `image` is a package path.
- `connections`: `{ id, kind, title, multiple? }`, with `kind` `oauth`, `api-key` or `agent-login`. `multiple: true`
  means several accounts with one active. The permissions a kind needs:
  - `api-key`: `secrets`.
  - `oauth`: `net` and `secrets`.
  - `agent-login`: `credentials`, and at least one `agentAdapters` entry. The agents session owns the sign-in.

**Category rule.** A declared category must agree with the contributions: `agents` needs an `agentAdapters` entry,
`skills` needs a `skills` entry, and `themes` needs a `themes` or `iconThemes` entry. `extensions` and `services` have no
requirement. When the category is absent it is derived in this order: `agents` if there is an agent adapter; else
`skills` if there are skills; else `themes` if the package holds theme kinds and nothing else; else `extensions`.
`services` is never derived, so it must be declared. The check is one-way: a declared `agents` category needs an
adapter, but an adapter does not force the `agents` category.

**Marketplace page data.** Each item of the page maps to a manifest field, a computed value, or runtime:

| # | Page item | Source |
|---|---|---|
| 1 | Logo | `listing.logo` |
| 2 | Name | `name` (display name), `description` (short description), `publisher` and `listing.developer` |
| 3 | Host | runtime: the host the item is installed on. Not in the manifest |
| 4 | Banner | `listing.banner.image`, `listing.banner.examples`, `listing.mention` |
| 5 | Connection | `listing.connections`, with the permissions each kind needs |
| 6 | Information | `version`, `api`, `requires.massorca`, `surfaces`, `modes`, `readme`; `category()` (declared or derived); `placements()`, the distinct slots of `contributes.views` in declaration order (the artboard's "Sidebar section" is the `sidebar-section` slot); the permission list with each reason and scope; the risk reasons |
| 7 | Settings (basic) | `contributes.settings` with `section` `basic` |
| 8 | Advanced Options | `contributes.settings` with `section` `advanced` |
| 9 | Danger Zone (uninstall) | `uninstall_effects()`, below |

The category names and the "MassOrca extras" developer label match `docs/design/artboards/bt-marketplace-catalogue.dc.html`
on main. "Third party" is a label the index source gives, not a manifest field.

### Permissions

The closed vocabulary of §6, typed. Scopes narrow a kind; a permission without a scope means "any":

- `files:read` and `files:write` take `paths`: globs relative to the workspace root.
- `net` takes `hosts`: exact hosts, `*.domain`, or `*` for any host.
- `secrets` takes `names`: secret names the extension may use by name.
- `process` takes `programs`: a program plus argument patterns, such as `git status *`. (This replaces the earlier
  "command patterns" wording and the `commands` field.)
- `credentials` takes `agents`: agent ids whose credentials may be used.

A kind without a scope rejects one. Each permission needs a non-empty `reason`. A kind declared twice is an error.

### Identifiers, paths and errors

Public so that the other sessions check the same rules: `ids` (kind ids, contribution ids, setting keys, extension ids,
handler names) and `package_path` (the file-path rule). The error type carries the codes that cross the protocol, all
under the reserved `extension.` prefix: `extension.invalid_manifest`, `extension.invalid_hash`,
`extension.hash_mismatch`, `extension.invalid_archive`, `extension.unsafe_path`, `extension.unsafe_entry`,
`extension.limit_exceeded`, `extension.missing_file`.

### Risk

Computed from permissions and contributions, never read from the manifest. Each reason names its subject and carries its
review copy.

- `files` with a whole-root glob (or no scope), and `net` to any host (or no scope), are High. Other file and host
  scopes are Medium.
- `process`, `credentials`, `terminals:write`, `agents:control` are High. `secrets`, `terminals:read`, `sessions:read`
  are Medium. `storage`, `notifications`, `workspaces:read`, `agents:read` are Low.
- `skills`, `agentRules` and `recipes` are High, because agents act on them with the owner's rights. Recipes are High
  whatever they contain, since their files are not read here.
- Each agent adapter and each MCP server is High. An adapter whose `cli.install` is non-empty adds the vendor-installer
  reason.

`review_update(old, new)` lists added permissions, widened scopes (added entries, or any scope), new risk reasons and the
risk before and after. `requires_review()` holds when any of them is non-empty, or the risk rises. Updates never widen
silently (§5).

### Uninstall

The manifest cannot declare what removal deletes. `uninstall_effects(manifest, cli_installed)` computes it:

- Always: the package files, the extension's stored data, and every setting key in every section.
- The secrets named by the `secrets` permission's scope. An unscoped `secrets` permission sets `secrets_unscoped`, so the
  caller enumerates its own store.
- The connection ids, whose stored sign-in is removed.
- The CLI binaries of the agent adapters, only when `cli_installed` is true.
- Offered, not deleted: the agent credential home (0056) of each agent the package provides.

### Archive

A `.mxt` is a zip. `archive::open(bytes, pinned_sha256, limits)`:

1. Verifies the SHA-256 first. A pin is 64 hex characters, either case; a mismatch refuses the install.
2. Refuses entries with absolute, `..`, drive, backslash or empty-segment names, symbolic links, exact duplicate names,
   and names that differ only in case (Windows would collide on extraction). Glob characters in names are allowed.
3. Caps the entry count (default 2000, directories included) and the total uncompressed size (default 64 MiB). Size is
   enforced while reading, so a lying header cannot exceed it.
4. Requires `extension.json` at the root, validates it, and checks that every referenced file is present.

It returns the manifest, the entry list and an in-memory `BTreeMap` of contents. It writes nothing.

### JSON Schema

The published JSON Schema export is deferred. The Rust types and rules are the source for now.

## Alternatives

- **Tolerate unknown fields** for forward compatibility. Rejected: a typo could silently grant or drop a permission, which
  §2 forbids.
- **Stop at the first semantic problem.** Rejected: the review and the author need every problem at once.
- **Let the manifest set the risk**, as a hint. Rejected: §6 has the core compute it.
- **Derive the uninstall effects from a declaration.** Rejected: a manifest could under-report what it deletes.
- **Use the `url` crate for `https` checks.** Rejected: a prefix and authority check is enough here and keeps the
  dependency list short.

## Consequences

- The crate depends on `semver` (with serde), `serde_path_to_error` (the JSON-pointer path on shape errors), `sha2`, and
  `zip` (deflate only, for the archive check).
- Other sessions read these types: `core-agents` takes the agent declaration, `core-extensions` writes the lockfile and
  runs the install, and the protocol carries the error codes. Their interfaces follow this crate's public API.
- Permission and kind vocabularies are closed. Adding one is a manifest change, reviewed with a contract bump.
- A command whose handler is a core method takes that method's permission risk (§6). That mapping needs the method
  registry, which this crate does not have, so it is not computed here.

### Differences from §6 and the brief

- The `process` scope field is `programs` (agreed with the agents session, which checks it against the kind's program
  candidates). `terminals` and `agents` kinds follow §6 (`terminals:read`, `terminals:write`, `agents:read`,
  `agents:control`). `credentials` is added as the brief asked; §6 does not list it, and it is High.
- Seventeen kinds, not sixteen (see above).
- `mcpServers` requires a `process` permission. The brief required that only for agent adapters.
- Duplicate permission kinds are an error, not a merge.
- The `themes` category requires at least one theme or icon theme. It does not forbid other kinds, which is a reading of
  "needs only themes or iconThemes" the owner may want to tighten.
- Commands in `listing.banner.examples` match literally, so an example text `/review` needs a command whose `keys` or `id`
  is `/review`.
