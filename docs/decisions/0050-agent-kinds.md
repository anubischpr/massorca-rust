# 0050: Agent kinds are declared external programs

Status: accepted, 2026-11-21.

## Context

Phase 4 (milestone M3) runs coding agents in workspaces. The old app knew 45
agent ids, each a third-party command-line program with its own flags for the
first prompt, resume and permission bypass (`docs/inventory/agents.md`,
AG-01, AG-04, AG-05). The extension model (0001,
`docs/architecture/extensions.md` §3 and §9) puts agent CLIs in its third
tier: a process the core runs, described by an `agentAdapters` contribution,
with built-in agents registered through the same point as extensions and
none special-cased in the launcher. The owner asked for four agents first:
Claude, Codex, Antigravity and Gentle Shell (rose icon).

## Decision

- **An agent kind is data.** A kind is a JSON declaration: `id`, `name`,
  `icon`, the `program` candidates, launch `args`, how a first prompt is
  given, how a session is resumed, which status mechanism it uses (0053),
  and which environment variable selects its credential home (0056). The
  launcher reads only the declaration; there is no `match` on kind ids.
- **Behaviour is a closed set of mechanisms.** Where a kind needs code (how
  to install status hooks, how to deliver a prompt) the declaration names one
  mechanism from a fixed list implemented in Rust (`claude-settings`,
  `codex-notify`, `env-command`, `none`; `argv`, `flag`, `none`). A new
  mechanism is a core change; a new kind that uses existing mechanisms is a
  data file. An unknown mechanism or field type makes the declaration
  invalid; it is never ignored.
- **Two sources, one registry.**
  - Built-in kinds ship as JSON files compiled into `core-agents`, owner
    `core`: `claude`, `codex`, `antigravity`, `gentle-shell`.
  - A profile may declare more in `<profile>/agents/kinds/*.json`, owner
    `user`. This is the process-runner tier of the extension model without
    packaging; at M6 an extension's `agentAdapters` entries load through the
    same loader with owner `ext:<id>`.
  - Ids are lowercase `[a-z0-9-]`, 1 to 32 characters. A profile file whose
    id equals a built-in id is invalid: it is listed with its problem and
    never replaces the built-in.
  - Declarations are read when the service starts and again on every
    `agent.kinds` call, so editing a file needs no restart.
- **Installed means resolvable.** A kind is installed when one of its
  `program` candidates resolves: an absolute path that is an executable
  file, or a bare name found on the service's `PATH` (on Windows with
  `PATHEXT`, so `claude.cmd` counts). No version probe in Phase 4. The
  service reports the resolved path; a kind that is not installed is listed
  as such and `agent.start` refuses it with `agent.not_installed`.
- **The four built-ins** (flags follow the inventory and are not yet checked
  against each vendor's current CLI; the opt-in smoke test of 0057 checks
  Claude on this machine):

  | id | name | icon | program | first prompt | resume | status (0053) | home variable (0056) |
  |---|---|---|---|---|---|---|---|
  | `claude` | Claude | `claude` | `claude` | argv | `--resume {ref}`; MassOrca assigns the ref with `--session-id` | `claude-settings` hooks | `CLAUDE_CONFIG_DIR` |
  | `codex` | Codex | `codex` | `codex` | argv | `resume {ref}`, ref learned from the turn-complete payload | `codex-notify` (turn end only) | `CODEX_HOME` |
  | `antigravity` | Antigravity | `antigravity` | `agy` | `--prompt-interactive` | `--conversation {ref}`, never learned yet, so not offered | none | none yet |
  | `gentle-shell` | Gentle Shell | `rose` | `gentle-shell` | argv | `-- --session {ref}`, never learned yet, so not offered | none | `PI_CODING_AGENT_DIR` |

  "Gentle Shell" is its name everywhere; it is never shown as "Pi".
- **No permission-bypass default.** Launch arguments are what the
  declaration and the caller give. MassOrca adds no "yolo" flag on its own;
  a caller who wants one passes it explicitly.

## Alternatives

- **Rust code per agent** (an enum of kinds). Every new agent would be a
  core release, and extensions could not add one through the same point.
- **Kinds stored in SQLite.** Declarations are files a person edits and an
  extension ships; the store would only mirror them.
- **Let a profile override a built-in id.** Convenient for pointing `claude`
  at another binary, but it makes "which Claude runs" depend on a file
  nobody sees. A per-kind program setting can come with the settings
  registry.

## Consequences

- Tests declare a `fake` kind in the test profile that points at the fake
  agent binary (0057), and exercise the same loader and launcher as Claude.
- Adding the remaining agents of the inventory is mostly data, plus a
  mechanism when one needs a new hook installer or prompt mode.
- Antigravity and Gentle Shell run and report `live` / `exited` but no
  activity until a status mechanism is declared for them.
