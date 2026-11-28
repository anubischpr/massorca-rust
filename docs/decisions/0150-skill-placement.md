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
  with a fresh scan when placing (a preview reuses the shared scan) and refuses any other kind (bundled, repository and
  plugin skills are not copied). The protocol never takes a free path for the source. The skill's folder name must match pv96's skill-name rule: lowercase letters, digits
  and hyphens, at most 64 characters.
- **Destinations.** pv96's global folders for the eleven agents: `.claude/skills`, `.cursor/skills`, `.gemini/skills`,
  `.factory/skills` (droid), `.continue/skills`, `.trae-cn/skills`, `.grok/skills`, `.augment/skills` (aug) and
  `.zcode/skills`. Codex and Muse read the canonical root and need no folder. Scope is `global` by default: these folders
  sit under the home folder. Scope `workspace` places in one workspace's root instead (see Workspace scope below).
- **Agents are chosen, not detected.** The CLI requires `--agent`. The desktop offers the agents whose folder exists on
  this computer (the discovery roots report that) and shows the folders before anything is written.
- **Alias first (pv96).** Each agent folder is an alias to the canonical copy:
  - on macOS and Linux, a relative directory symlink, computed between the resolved parent and the resolved canonical
    copy so a linked folder above it (a stow-managed `~/.claude`) does not break it. The link must resolve to the
    canonical copy once made; if it does not, it is removed and the folder is copied instead;
  - on Windows, a directory junction with an absolute target, made and read with the `junction` crate 2.1.0
    (Windows-only, in `massorca-core-extensions`). The standard library cannot make a junction; the crate's API is safe,
    so the workspace's ban on `unsafe` holds and no process is spawned. Removal uses `RemoveDirectory`, which deletes the
    junction and not its target.
- **Verified copies.** The canonical copy is checked against the source's digest once made, and removed with an error
  when it does not match.
- **Verified copy as the fallback.** A folder becomes an independent copy only when its alias is refused or unavailable.
  The copy must match the source's digest before it counts as placed; otherwise it is removed and reported
  (`skipped_copy_failed`).
- **Topology is recorded.** The managed record stores each agent folder's topology: `canonical` (the agent reads the
  canonical root), `alias` or `copy`.
- **Removal checks the topology before deleting.** The record never names what is deleted: removal recomputes each
  agent's folder from the home folder, the agent and the skill name, and a row whose recorded path differs is kept and
  reported (`skipped_modified`). An alias is removed only while its link still points at the canonical
  copy, and it is removed as a link: the target is never deleted through it. A copy is removed only while its digest still
  matches. A changed alias or copy is kept and reported (`skipped_modified`). The canonical copy goes only when no placement
  still reaches it; an agent that reads the canonical root and is unplaced while another agent keeps the copy is
  reported as `canonical`, since it still reads it.
- **Never overwrite.** A folder that already exists and that MassOrca did not place is skipped and reported
  (`skipped_existing`). A canonical copy the user made refuses the whole placement (`skill.name_taken`), so no agent
  folder points at a copy MassOrca does not own. A link is created only where no path exists, so an unowned folder is
  never replaced.
- **Workspace scope.** A placement for a workspace uses the same canonical copy and folder rules, with the workspace's
  root in place of the home folder: `<root>/.agents/skills/<name>` and each agent's folder under `<root>` (for example
  `<root>/.cursor/skills/<name>`). The request names the workspace by its MassOrca id, never by a free path. The service
  resolves the id to the workspace's folder on the host that owns it, matching the id exactly; a workspace being removed
  is refused (`workspace.not_found`) and a workspace whose folder is missing is refused (`path.not_found`). A git worktree
  and a folder workspace are both roots, since only the folder matters.
- **Switch agents.** `skills.agents` adds agents to, and drops agents from, a placed skill in one scope, under the same
  lock as the other writes. Adding creates only the new agents' folders, placed from the canonical copy; the agents
  already placed are left as they are. Dropping removes only the dropped agents' copies, under the removal checks above;
  the canonical copy stays while another placement still reaches it. A skill that is not placed in that scope is refused
  (`skill.not_placed`); an agent named in both lists, or no agent at all, is refused (`request.invalid_params`).
- **Managed record.** `<profile>/skills/placements.json` records each placed skill, its scope (no workspace is global, so a record from before scopes reads as global), its digest, and each agent's folder
  and topology. The record is replaced atomically; an unreadable record, or one holding a skill name the name rule refuses, refuses
  removal (`skill.record_invalid`).
- **Digest.** The digest covers every directory and file by relative path and bytes. A source whose digest differs from
  the recorded one is refused (`skill.content_changed`) until its placed copies are removed.
- **Sweep.** Staging folders named `.<skill>.placing-<pid>-<nanos>` that a crash left beside a placement are removed
  before the next placement or removal of that skill. Only plain folders with exactly that name pattern, whose recorded
  time is at least ten minutes old, are removed, so another process's copy in progress stays.
- **Safety.** A link, a Windows junction or another reparse point, or a non-file entry inside a skill refuses the
  placement. A skill is limited to 2,000 files, 64 MiB and 16 folders deep.
- **Permissions and capability.** `skills.place-preview` and `skills.managed` read; `skills.place` and `skills.unplace`
  need host administration. The capability is `skills-place/1`, separate from `skills/1`.
  `skills-place/2` adds the workspace scope (the optional `scope` and `workspace_id` fields of the place, unplace and
  preview params) and `skills.agents`, which needs host administration too. A field a service of `/1` does not know is
  ignored, so a client sends `scope: workspace` only to a service that offers `/2`; a client that sends nothing gets
  `global`, and a `scope` value this build does not know is refused.

## Alternatives

- Route placement through the extension install and its `skills` contribution. Rejected for now: the install flow has
  review and consent for packages, not for a link or copy into other agents' folders, and pv96 keeps the two apart.
- Accept a free path as the source. Rejected: an administrator's client could then link or copy any folder into agent
  folders.
- Create the Windows junction by spawning `mklink /J`. Rejected: it runs `cmd.exe` with paths as free text, so a path
  holding `&`, `^` or `%` becomes command injection.

## Consequences

- Placed skills are visible to every agent listed. An agent that edits its alias edits the canonical copy, as pv96 does.
- The record lives in one profile; placements made by another profile are not known to this one.
- Not built: bundle install and its update (SKL-03, SKL-06, SKL-07), provider root overrides, and
  paired-host, SSH and WSL placement (SKL-15 to SKL-17).
- On Unix a narrow race remains: a folder created in the instant before the placement can be replaced if it is empty. The
  check before the placement narrows the window; it does not close it.
