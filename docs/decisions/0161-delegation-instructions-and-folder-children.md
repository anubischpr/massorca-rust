# 0161: Delegation instructions and folder children

Status: accepted, 2026-11-25.

## Context

- An agent run could call `massorca agent delegate`, but nothing told it the command existed, and `massorca` was on
  its `PATH` only when the person had put it there. Asked for "a child session", an agent used its own subagents,
  which the person cannot see in the Sidebar.
- The previous product (pv96) prepends its CLI folder to the agent's `PATH` and gives each run a short instruction
  text, only when the run carries its session identity, through a channel per vendor: Claude's
  `--append-system-prompt`, Codex's app-server `developerInstructions`, an `AGENTS.md` in an app-owned folder for
  Antigravity, Pi's `systemPrompt`.
- A writing child of a plain folder workspace was refused with `agent.delegate_needs_git`: it could not get a worktree
  of its own. pv96 lets such a child share the folder (placement `current`, warning `folder-workspace-writes`, no
  lock).
- Decision 0072 allows one writing session per workspace (`agent.workspace_busy`), unless the folder is marked to share
  writers.

## Decision

- Every agent run's `PATH` starts with the folder of the `massorca` executable the service runs (`MASSORCA_BIN`'s
  folder), with the platform's separator. The rest of the `PATH` (the caller's or the vault's when set, else the one
  inherited from the service) is kept, and the folder is not added again when it is already first.
- A kind may declare an instruction channel as data: `"instructions": { "args": [..., "{text}"] }`, launch arguments
  holding `{text}` exactly once and no other placeholder (checked by `problems`). The text goes on the run's own
  command line, on a first run and on a resume, never into the vendor's configuration.
  - The built-in `claude` declares `["--append-system-prompt", "{text}"]`.
  - `codex`, `antigravity` and `gentle-shell` declare none: pv96 reaches them through an app-server parameter, a
    file in an app-owned folder and an SDK option, none of which is a flag of a terminal run, so they get no text.
- The text is built by the service, short, in English, on one line without quotes or `<`, `>`, `|`, `&`, `%`, `^`, so
  a Windows `.cmd` shim passes it as one argument. It says to run `massorca agent delegate` when the person asks for
  a child session, a delegated session, or work in its own session or worktree; names `agent tree`, `agent status
  --until done`, `agent status --json`, `agent stop` and `agent bring-back`; and says that the agent's own subagents
  are a different thing, still allowed, that the person cannot see in the Sidebar. A run gets it only when
  `agents.delegation_depth` leaves room for a child below its session; every MassOrca run sets the session identity.
- Support probe: a channel may declare `supported_when: { args, contains }`. The service runs the kind's resolved
  program with `args` (through `massorca_process::command`, at most 5 s, no account home, the names an account command
  strips removed, never a login) and uses the channel only when it exits 0 and its output contains `contains`. The
  result is cached per program path and modified time, so it runs once per installed CLI, not per launch. The built-in
  `claude` probes `--help` for `--append-system-prompt`. When the probe fails, times out or lacks the flag, the run
  starts without the text and the session's `notes` say "Delegation instructions were not given: this Claude CLI does
  not support --append-system-prompt." (the kind's name and the probed flag). The note is kept in the run directory
  and cleared by a later run that gets the text.
- Folder children, an exception to decision 0072 for a delegated tree:
  - From a git worktree workspace, a writing child gets its own worktree, as before.
  - From a plain folder workspace, a writing child runs in the parent's folder, with no copy. The delegation tree the
    parent belongs to (the sessions under the same root) counts as one writer: `agent.workspace_busy` does not refuse
    a writing start or resume of a session of that tree because another session of the tree writes there. Any other
    writing start in the folder is refused as before. `agent.delegate_needs_git` no longer refuses this case; it stays
    for a worktree with no branch.
  - The child's start result (`note`) and its record (`notes`) say, verbatim: "This workspace has no Git, so
    the child works in this folder."
  - Refusals that stay: `agent.depth_exceeded`, a busy workspace held by a session outside the tree, and the existing
    secret and scope guards.
- Every session view carries `delegation: { writing_child, read_only_child, reason? }`: `writing_child` is
  `own_worktree`, `shared_folder` or `unavailable`; `read_only_child` is false only when no child is possible (the
  depth limit); `reason` says in plain words why a writing child is unavailable. The field is additive and absent from
  older peers. `notes` (a list of plain-word notes, absent when empty) carries the folder note and the probe note.

## Consequences

- An agent asked for a child session calls `massorca` without the person setting anything up.
- In a folder, a parent and its children may write the same files at once. The person asked for that work, and the
  note says where the child works.
- Codex, Antigravity and Gentle Shell runs are not told about delegation until a terminal channel for them is known.
