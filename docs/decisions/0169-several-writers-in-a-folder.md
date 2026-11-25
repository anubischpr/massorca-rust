# 0169: Several writers in a folder

Status: accepted, 2026-11-25.

## Context

- Decision 0072 allows one writing session per workspace (`agent.workspace_busy`). Its amendments let a team folder and a
  delegated tree in a plain folder take several writers (0072 amendment of 2026-11-22, 0161).
- The owner could not start a second agent session in a folder project (no Git) while the first one ran. The desktop
  reuses the project's folder workspace, so the service refused with `agent.workspace_busy`.
- The reason for one writer is a git index that two writers could corrupt. A folder has no git index, so that reason
  does not reach it. The folder is the person's own, like two terminals open in the same directory.

## Decision

- A folder workspace takes several writing sessions at once. `agent.start`, `agent.chat.start` and `agent.resume` no
  longer refuse a writing session because another writing session runs in the same folder workspace.
- A worktree workspace keeps the refusal exactly as before, including the delegated-tree exception of decision 0161
  (which now matters only for a folder, so it is no longer consulted). A second writing start in a worktree still fails
  with `agent.workspace_busy` and names the holding session in `data.session`.
- The one-writer rule is decided by the workspace kind. Only a worktree keeps it. A kind a newer peer adds keeps the
  refusal until it is named as a folder. This build has no in-place kind, so none is treated as a folder yet.
- The shared-writers mark of decision 0072 (`shared_writers`, set by the team folders) is no longer needed for a
  folder. It is kept in the store and unchanged, so nothing that reads it breaks.
- `shared_folder_with` is an additive field of `AgentSession`: the other live writing sessions of the folder
  workspace, in the store's order. It is omitted when empty. It is never set for a worktree, a read-only session or an
  exited session. A client shows "Another session is also writing in this folder." when it is non-empty.
- No capability. The refusal is only relaxed, so an older client keeps working; it simply does not show the note.
- Read-only sessions are unchanged: they never refuse a writer and are never listed.
- Bring-back (0055) still checks the parent worktree for another live writer, since a merge needs a worktree.

## Consequences

- Two sessions in one folder can edit the same file at once. The service adds no lock and no merge; that is the
  point of the rule, and the owner accepts it.
- Each session view in a folder reads the execution of its sibling sessions, so a list costs one such read per
  sibling. Folders hold few sessions in practice.
- The desktop does not show the note yet. Its change is a follow-up; this decision needs only the field.
