# 0150: Skill placement

Status: accepted, 2026-11-25. Covers SKL-02 in `docs/inventory/skills-and-plugins.md`, on top of the discovery record 0088.

## Context

SKL-01 lists the skills agent tools keep on this host. SKL-02 makes an installed skill visible to other agent tools: one
canonical copy in `.agents/skills/<name>`, then an alias in the folder of each agent that reads its own folder. The
reference app (pv96, behaviour only) does this in `src/main/skills/skill-provider-destinations.ts` and its install
service, and it places an alias first and a verified copy only when an alias is refused. It takes a skill from a catalog
bundle (SKL-06 and SKL-09, not built here), so the source of a placement is a different question here. The extension
package format already carries `skills` folders (decision 0061), but pv96 installs skills through their own methods, not
through the extension install, so placement has its own methods.

## Decision

- **Source.** The skill is a home skill that discovery lists, named by its discovery `id`. The service resolves the id
  with a fresh scan and refuses any other kind (bundled, repository and plugin skills are not copied). The protocol never
  takes a free path for the source. The skill's folder name must match pv96's skill-name rule: lowercase letters, digits
  and hyphens, at most 64 characters.
- **Destinations.** pv96's global folders for the eleven agents: `.claude/skills`, `.cursor/skills`, `.gemini/skills`,
  `.factory/skills` (droid), `.continue/skills`, `.trae-cn/skills`, `.grok/skills`, `.augment/skills` (aug) and
  `.zcode/skills`. Codex and Muse read the canonical root and need no folder. Scope is global only; a workspace scope is
  not built.
- **Agents are chosen, not detected.** The CLI requires `--agent`. The desktop offers the agents whose folder exists on
  this computer (the discovery roots report that) and shows the folders before anything is written.
- **Alias first (pv96).** Each agent folder is an alias to the canonical copy:
  - on macOS and Linux, a relative directory symlink;
  - on Windows, a directory junction with an absolute target. The standard library cannot make a junction and the
    workspace forbids `unsafe`, so the junction is made with `mklink /J` through `massorca_process`, with an argument
    vector and no shell string. Removal of a junction uses `RemoveDirectory`, which deletes the link and not its target.
    The per-operation process spawn is the cost of this choice, and it is recorded here for the EDR review that the
    Windows rules call for.
- **Verified copy as the fallback.** A folder becomes an independent copy only when its alias is refused or unavailable.
  The copy must match the source's digest before it counts as placed; otherwise it is removed and reported
  (`skipped_copy_failed`).
- **Topology is recorded.** The managed record stores each agent folder's topology: `canonical` (the agent reads the
  canonical root), `alias` or `copy`.
- **Removal checks the topology before deleting.** An alias is removed only while its link still points at the canonical
  copy, and it is removed as a link: the target is never deleted through it. A copy is removed only while its digest still
  matches. A changed alias or copy is kept and reported (`skipped_modified`). The canonical copy goes only when no placement
  still reaches it.
- **Never overwrite.** A folder that already exists and that MassOrca did not place is skipped and reported
  (`skipped_existing`). A canonical copy the user made refuses the whole placement (`skill.name_taken`), so no agent
  folder points at a copy MassOrca does not own. A link is created only where no path exists, so an unowned folder is
  never replaced.
- **Managed record.** `<profile>/skills/placements.json` records each placed skill, its digest, and each agent's folder
  and topology. The record is replaced atomically; an unreadable record refuses removal (`skill.record_invalid`).
- **Digest.** The digest covers every directory and file by relative path and bytes. A source whose digest differs from
  the recorded one is refused (`skill.content_changed`) until its placed copies are removed.
- **Sweep.** Staging folders named `.<skill>.placing-*` that a crash left beside a placement are removed before the next
  placement or removal of that skill. Only plain folders with that name pattern are removed.
- **Safety.** A link, a Windows junction or another reparse point, or a non-file entry inside a skill refuses the
  placement. A skill is limited to 2,000 files, 64 MiB and 16 folders deep.
- **Permissions and capability.** `skills.place-preview` and `skills.managed` read; `skills.place` and `skills.unplace`
  need host administration. The capability is `skills-place/1`, separate from `skills/1`.

## Alternatives

- Route placement through the extension install and its `skills` contribution. Rejected for now: the install flow has
  review and consent for packages, not for a link or copy into other agents' folders, and pv96 keeps the two apart.
- Accept a free path as the source. Rejected: an administrator's client could then link or copy any folder into agent
  folders.
- Create the Windows junction through an `unsafe` FFI call or a new crate. Rejected: the workspace denies `unsafe`, and no
  junction crate is in the lock.

## Consequences

- Placed skills are visible to every agent listed. An agent that edits its alias edits the canonical copy, as pv96 does.
- The record lives in one profile; placements made by another profile are not known to this one.
- Not built: bundle install and its update (SKL-03, SKL-06, SKL-07), workspace scope, provider root overrides, and
  paired-host, SSH and WSL placement (SKL-15 to SKL-17).
- On Unix a narrow race remains: a folder created in the instant before the placement can be replaced if it is empty. The
  check before the placement narrows the window; it does not close it.
