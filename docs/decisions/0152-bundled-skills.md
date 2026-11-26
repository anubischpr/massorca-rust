# 0152: Bundled skills

Status: accepted, 2026-11-26. Covers SKL-04 in `docs/inventory/skills-and-plugins.md`, on top of skill placement (0150).

## Context

SKL-04 ships MassOrca's own skills to the agent tools. The reference app ships eight skills that load guides from its CLI.
Here the first version ships one skill, `massorca`, which teaches an agent the `massorca` command of the build on this
computer. Its command reference must match that build: an agent that follows a command the installed build refuses
fails for the person. Placement (0150) already copies a home skill into every agent folder; a bundled skill needs the
same placement, but its source is text the binary carries, not a folder the person owns.

## Decision

- **Text.** `packages/skills/massorca/SKILL.md` is embedded in the binary (`include_bytes!`, as the built-in catalogue
  embeds `packages/`). Its command reference is generated from the CLI's clap definitions (`massorca_cli::skill_guide`):
  one bullet per command with its arguments and the values its options take, and the build's version in the text. A unit
  test fails when the checked-in file differs from what the build renders. `massorca skills bundled show massorca`
  prints that render, so regenerating the file is a redirect.
- **Source.** The placement engine takes a folder. For a bundled skill the host writes the embedded files into
  `<profile>/skills/bundled/massorca/` and places that folder. The protocol takes a bundled name, never a path, so a
  client with host administration cannot place another folder through it.
- **Protocol.** Capability `skills-bundled/1`. `skills.bundled.list` reads each skill's name, description, version,
  state (`not_placed`, `current` or `outdated`) and agents. `skills.bundled.install` and `skills.bundled.refresh` need
  host administration and answer with the `skills.place` result. Removal is `skills.unplace`, so no method is added for it.
- **Agents.** An install that names no agent places for every agent whose skill root exists on this host, the rule the
  Marketplace uses to offer agents. A host with none refuses and asks for an agent.
- **Version and refresh.** The record keeps the digest of the placed folder, which covers the text. A build whose text
  differs reports the skill `outdated`. Refresh removes the placed copies the way `skills.unplace` does, keeping a copy
  the person changed, and places this build's text for the agents the skill had. When a changed copy keeps the old text,
  nothing new is placed and the result says what was kept. Refresh never runs at startup: it writes into agent folders,
  so it runs when the person asks, from the CLI or the Marketplace's Update.
- **Digest.** The folder digest is fed one entry at a time (`FolderDigest`), so a folder held in memory digests the same
  as the folder written from the same bytes. The state of a skill is read without writing anything.
- **Marketplace.** The Skills section lists "MassOrca skills" above the discovered skills, each with its state and one
  action. Install and Update show the review that placement shows, naming the agent tools, and send the call after the
  person confirms. Remove uses the existing removal review.
- **Listing read.** The Marketplace's page-open reads of placed and bundled skills now use the placement request
  numbers, which change refreshes also use. Before, a read could be dropped as older than a change and the list stayed
  stale. This also applies to placed skills of SKL-02.

## Alternatives

- Ship the skill through the extension catalogue, like the agent and tool packages. Rejected: an extension install has
  review and consent for packages, not for writes into agent folders, and 0150 keeps placement apart from it.
- Write the reference by hand. Rejected: it drifts from the CLI, and the generated text with its test cannot.
- Refresh at service start. Rejected: it writes into the person's agent folders without a request. The Marketplace shows
  the update instead.

## Consequences

- An installed copy is `outdated` after each build whose text changes, which includes a new version, until refresh.
- Not built: the `extension-author` skill of `docs/architecture/extensions.md` section 11. Creator mode has no service path
  yet, so there is no session to load it into. Also not built: further bundled skills, guides with reference files
  (`skills get <topic>`), workspace scope, and install or update on a paired host or over SSH (SKL-15 to SKL-17).
- A skill of the same name that the person owns blocks the placement the way it blocks SKL-02: a canonical copy they own
  refuses the install, and an agent folder they own is skipped and reported.
