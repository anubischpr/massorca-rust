# 0153: Tool, skill, view, theme and status-rule extension points

Status: accepted, 2026-11-26. Covers CUS-08 in `docs/inventory/coverage.md`, on top of the contribution registry of
`docs/architecture/extensions.md` §9, the install and removal of 0062 and 0098, skill placement (0150) and host programs
(0151).

## Context

The core already had a `Registry<K>` whose entries carry an owner and return a drop handle, and used it for methods,
capabilities, commands and settings. The event bus, the command table and agent kinds were in place, and features
registered remove guards and cleanups on the extension service. Packages could already declare `tools`, `hostPrograms`,
`skills`, `views`, `themes`, `iconThemes` and `agentRules`, but nothing in the core registered them, so nothing could
ask which package offers a theme or drop a removed package's view.

## Decision

- **Points.** Five more registries of the same `Registry<K>`: tools, skills, views, themes and status rules. No parallel
  registry: each entry has an owner and a drop handle like every other contribution.
  - Tools hold the MCP `tools` a package serves and the `hostPrograms` it installs (0151), each marked with its source.
  - Skills hold the package's skill folders. Skill placement (0150) resolves a skill id that is a point key from this
    point, so `skills.place` copies a package's skill from its trusted package folder into the agents' folders.
  - Views hold the view document and its slot; themes hold colour themes and icon themes, marked apart.
  - Status rules hold the `agentRules` documents (CUS-05) for agent status.
- **Keys and owners.** A key is `<package id>/<contribution id>`; the owner is `ext:<package id>`. Core contributions, when
  there are any, use owner `core` and their own keys.
- **Owner-scoped handles.** The handles a package's contributions return are kept together per owner. Dropping the owner
  drops every handle, and every point loses that package's entries at once. A package registers all of its contributions
  or none: the first refusal drops what it had registered so far, and the package is reported as a problem with code
  `extension.point_refused`.
- **Which packages.** Only enabled packages whose files match their lockfile hash register. A package whose files cannot
  be trusted registers nothing and is reported with its own code (`extension.tampered`). A first-party id the catalogue
  does not vouch for registers nothing, as for agent kinds (0098).
- **When.** The points sync with the enabled packages at service start, after each install, update, rollback,
  enable, disable and removal, and before `extensions.points` answers. A sync drops owners that are gone, disabled or
  moved to another folder (an update), and registers the rest.
- **Removal.** A new remove listener on the extension service (`register_remove_listener`, beside the guards and
  cleanups) hears every committed removal, whatever `delete_agent_homes` says. The points register one that drops the
  removed package's owner, so a removal from any caller empties the points without waiting for a sync.
- **Debug entry point.** `extensions.points` (capability `extension-points/1`, read permission) lists each owner with the
  keys it holds on each point, and the packages that registered nothing. `massorca extension points` prints it.

## Alternatives

- Read the lockfile on every use of a point, as agent kinds do (0063). Rejected: it hashes every package's files per
  read, and a drop handle per owner already says when a contribution ends.
- Drop registrations from the remove cleanups. Rejected: cleanups run only when `delete_agent_homes` is asked for.
- Place a package's skills automatically on install. Rejected: placement writes into the agents' folders, which needs the
  person's choice of agents (0150).

## Consequences

- Skills a package placed stay in the agents' folders after the package is removed; `skills.unplace` removes them, as
  for any placed skill. Only the point entry goes with the package.
- Tools are registered but not served to agents yet; the MCP server that serves them reads this point when it is built.
  Views, themes and status rules have no consumer yet either; their consumers read these points.
- Every wire change is additive: one new capability, one new method and its new types, and one new error code.
