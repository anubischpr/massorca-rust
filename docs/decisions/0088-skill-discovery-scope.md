# 0088: Skill discovery scope

Status: accepted, 2026-11-24. Covers the first three steps of SKL-01 in `docs/inventory/skills-and-plugins.md`.

## Context

SKL-01 lets the owner see which skills agent tools already keep on this host. The reference app (pv96, behaviour only)
scans the home folders of about twenty agent tools, the shared `.agents` folder, the plugin caches, each repository's
skill roots and the current directory. Its root table is `src/main/skills/skill-discovery-sources.ts`. The Skills page
and the composer menu come later. This record covers the core scan, the protocol method and the CLI.

## Decision

- **Root table.** The roots are pv96's: 21 home-level roots (the Codex plugin cache and the Antigravity CLI and built-in
  folders among them), and seven roots per registered folder (`.agents`, `.claude`, `.factory`, `.continue`, `.trae`,
  `.grok`, `.augment`). Enabled Claude plugins add their roots too. A plugin contributes one install for each context:
  a local or project install only inside its project, otherwise the user install, the most specific first. The contexts
  are the home folder and each registered folder.
- **Depth and reads.** A `SKILL.md` counts when it sits at most five folders below a home or folder root, or ten below a
  plugin cache. Only the first 256 KiB of a file are read. A `.system` folder under a home root is bundled. A skill that
  several roots reach is listed once, with every root that reached it.
- **Bounds.** One scan examines at most 10,000 directory entries across all its roots (`MAX_SCAN_ENTRIES`). When the
  budget runs out, the root being read is reported `truncated`, and so is every later root with entries, because none of
  their entries are examined; the skills listed
  are the ones found before that. A frontmatter description keeps at most 240 UTF-16 units, the same limit the fallback
  paragraph stops at. A scan is shared for ten seconds, and a caller that asks for a refresh while a scan of the same
  request runs joins that scan instead of starting another. pv96 bounds its Claude command folder scan with the same kind
  of per-scan entry budget, at 1,000 entries; the 10,000 here is MassOrca's own choice.
- **Skipped roots.** Not built yet: the Antigravity plugin roots (they need its config reader), the Hermes fallback to
  `%LOCALAPPDATA%\hermes` on Windows (Hermes uses `~/.hermes/skills`), the 128-root cap on repository roots, marketplace
  source labels, WSL and remote scans, and the CLI's current directory (folders come from registered projects, because
  the service may run on another host). The `names` filter and the staging-name skip are left out: no install or delete
  staging exists yet.
- **Path hiding.** `skills.discover` is a read method. Host paths (roots and skill files) reach only a caller with host
  administration. Other readers get labels, kinds, existence and skip reasons. The extension list hides the paths of
  detected programs the same way.
- **`MASSORCA_SKILL_HOME`.** The service takes its home folder from `MASSORCA_SKILL_HOME` when it is an absolute path,
  and from the platform's home folder otherwise. Tests and diagnostics set it, as they set `MASSORCA_VAULT_PROTECTION`.
  It is not a user setting. `CLAUDE_CONFIG_DIR`, `GROK_HOME` and `HERMES_HOME` are read the same way, and a relative
  value is ignored.
- **The name `skills roots`.** The root report is `massorca skills roots`, and `massorca skills installed` lists the
  skills. In pv96 `skills list` lists the bundled guides (SKL-04), so that name stays free.

## Alternatives

- Keep `skills list` for the root report. Rejected: SKL-04 needs the name for the bundled guides.
- Show host paths to every reader. Rejected: any paired device with read access would see the host's folder layout.
- Scan the CLI's current directory. Rejected for now: the service cannot see the directory of a CLI that runs against
  another host.

## Consequences

- A paired reader sees which skills exist and where they came from by label, but not where they are on the host.
- The root table follows pv96. A new agent tool needs a root in the table and a line in this record.
- The skipped roots are gaps for SKL-01 to close, not decisions to leave them out.
