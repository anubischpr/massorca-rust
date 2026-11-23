# 0063: Built-in agent kinds are packages

Status: accepted, 2026-11-22. Builds on 0059 (agents come from the Marketplace),
0061 (package format) and 0062 (tier-1 install). It carries out 0059's second
step for the four built-in kinds.

## Context

0062 installs the four first-party agent packages, `massorca.<kind>`, from the
built-in catalogue. Nothing reads them yet. `core-agents` still compiles the
four declarations in and counts them as installed, so it cannot tell an
available kind from an installed one. A profile that already used one of the
kinds (sessions, accounts, bots, credential homes) must keep working when the
switch happens. Nothing may be lost, and no run may break silently.

## Decision

### Two sides of one seam

- `core-extensions` owns the packages. It exposes plain functions with no
  `core-agents` dependency:
  - `package_agent_kinds(&Profile)` lists every kind of the catalogue, then the
    kinds of installed third-party packages in lockfile order. A built-in with
    an installed copy is listed as that copy, at its catalogue position. Each
    entry has `kind_id`, `package_id`, `version`, `state` and `decl`. `decl` is
    the declaration text, or an error that names the package and the cause and
    never a path.
  - `adopt_builtin_agents(&Profile, &BTreeSet<String>)` runs the migration
    below.
- `core-agents` implements `KindPackages::kinds()` by calling these functions
  and maps `state` to the protocol's `PackageState`. `core-extensions` does not
  depend on `massorca-protocol`, so `state` is its own `item::ItemState`, which
  has the same three values.

### States

- `Available`: not installed. A built-in's declaration is the embedded one.
- `Enabled`: installed and enabled.
- `Disabled`: installed and not enabled.

Removing a built-in returns its kind to `Available`, because the catalogue lists
it again. A third-party package is listed only while it is installed.

### Gating

A run needs its kind installed and enabled. A kind that is not `Enabled` refuses
these with `agent.kind_not_enabled`:

- starting a session, including a delegated child session;
- resuming a session;
- adding, importing or activating an account of the kind.

Removing the kind, checking it and signing out stay allowed.

### Install and uninstall

- Installing a built-in copies its package from the binary and downloads
  nothing. It registers a CLI already on the machine. A vendor command runs only
  after explicit consent, and only where the declaration names one and verifies
  it (0062).
- Uninstall in the Danger Zone removes the registration: the lockfile entry and
  the package folder. The vendor CLI stays, unless the person asks through the
  uninstall command shown for it (0062, `--uninstall-cli`).

### Migration for existing profiles

The server runs `adopt_builtin_agents` once per profile, at startup. Its input,
`used_kinds`, holds every kind id the profile used: sessions, accounts
(including pending ones), account history, bots and credential-home folders.
Kind ids that are not built-in are ignored.

- For each built-in package that declares a used kind and is not installed, the
  migration registers the package as `Enabled`, with `adopted: true`. It runs no
  vendor command, resolves no program and asks for no consent, because the build
  vouches for the package.
- It records the run as `migrations: ["builtin-agents"]` in `lock.json`, in the
  same atomic write as the adopted entries. Later calls change nothing, so a
  built-in that the person removes afterwards is never adopted again. An empty
  `used_kinds` still records the run.
- A failure, such as a busy package, a damaged lockfile or a write error, leaves
  the lockfile unchanged and the run unrecorded, so the next start tries again.
- The free function builds its own service, so its change lock is not shared
  with services already running on the same profile. The server must therefore
  run it before any service of that profile changes anything, or call the
  service's `adopt_builtin_agents` method instead.

Both lockfile fields are additive. An older service keeps unknown fields on
write, so it still works.

### Compiled-in copies

Under 0059, the copies compiled into `core-agents` (`crates/core-agents/kinds/`)
are removed in the same change that makes the server read packages. Until then,
the catalogue test `agent_declarations_equal_the_core_agents_copies` keeps the
embedded declarations equal to them.

### Wire

No wire change. The server change adds the `KindPackages` implementation and the
protocol mapping.

## Consequences

- The server switches its loader to `package_agent_kinds` and drops the
  compiled-in kinds in the same change.
- A profile that used a built-in kind keeps its kind, and nothing it had is
  deleted by the migration.
