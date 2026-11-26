# 0117: Forgetting a session with its workspace runs the archive script

Status: accepted, 2026-11-25. Extends 0095 and 0163.

## Context

`workspace.remove` runs the project's trusted `orca.yaml` archive script in the worktree before anything is stopped or
removed (decision 0095). `agent.forget` with `remove_workspace` (decision 0163) removes the same worktree but skipped that
script, so a session's workspace could be deleted without the work-saving step the project had declared.

## Decision

**D1, archive on forget.** `agent.forget` with `remove_workspace` runs the workspace's archive script in the same place
as `workspace.remove`: after `workspace.removing`, before the worktree is removed. An untrusted or absent script is skipped
as before. The archive step is shared, not duplicated.

**D2, ordering.** Decision 0163's order holds: every check runs first, then the session is forgotten, then the removal
begins. An archive refusal therefore comes after the forget, and is returned in `workspace_error` with code
`workspace.archive_failed`. The removal is cancelled and the workspace returns to `ready`, so the worktree and its row stay,
and `workspace.remove` can remove them later. The session record is already forgotten; this is the same outcome 0163
gives every removal refusal after the forget.

**D3, waiver.** `agent.forget` gains an optional `allow_failed_archive` (wire-compatible; absent means false), mirroring
`workspace.remove`. With it, a failed archive is reported in `workspace_removed.archive` and the removal goes ahead. The CLI
flag is `agent forget <SESSION> --remove-workspace --allow-failed-archive`; it requires `--remove-workspace`. Clients that
send it use capability `agent-archive/1`, as for the other removal options.

## Consequences

- A user who forgets a session with its workspace now gets the project's archive script run first, as with
  `workspace rm`.
- A failed archive can leave a session forgotten while its workspace remains. Remove the workspace with
  `workspace rm`, optionally with `--allow-failed-archive`.
</content>
