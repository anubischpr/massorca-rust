# 0098: Removing a workspace with delegated children

Status: accepted, 2026-11-24.

## Context

A writing child of a delegation gets a workspace of its own, `<parent>-<n>` (0055); a read-only child runs in its
parent's workspace. Removing a parent workspace (0008) left the child workspaces behind, with their worktrees and
branches, and nothing pointed at them any more (WS-16).

pv96 does not remove child worktrees with their parent: it keeps a lineage record per worktree and prunes the records
whose worktrees are gone. The child worktrees stay until removed one by one.

## Decision

- The child workspaces of W are the workspaces of agent sessions whose parent session runs in W and whose own
  workspace is not W, and recursively theirs. The store answers it with one read-only query, deepest first; a read-only
  child adds none.
- The parent is checked first. When a workspace has child workspaces, its own refusals (main worktree, locked, orphan,
  dirty, ignored files) run first, read-only, with the caller's waivers. A refusal is returned as is, ahead of
  `workspace.has_children` and with or without `with_children`, and no child is removed.
- Otherwise `workspace.remove` of a workspace with child workspaces is refused with `workspace.has_children`. The
  message and `data.children` name them, deepest first. Nothing is changed. The CLI rewords it to name
  `--with-children`.
- A new waiver, `with_children` (`massorca workspace rm --with-children`), removes the children first, deepest first.
  Each goes through the ordinary removal: its checks, the `workspace.removing` event, stopping its runs and terminals,
  the removal and the `workspace.removed` event. The caller's other waivers (`force`, `discard_ignored`,
  `forget_orphan`) apply to every child; `with_children` covers only the children, never another refusal.
- A refused child stops the removal. The error keeps the child's code (for example `workspace.dirty`); its message and
  `data` name the child (`data.child`) and the children already removed (`data.removed_children`), which stay
  removed. The parent is not touched.
- A child workspace already gone from the store is skipped. One whose directory is gone is removed like any missing
  workspace.
- The result carries `removed_children`, the removal results of the children, deepest first; absent when none.

## Consequences

- `with_children` and `removed_children` are optional fields, so an older client neither sends nor reads them; an
  older client meets the new refusal code as an unknown error with a readable message.
- Stopping the parent's runs happens only when its own removal begins, after the children, so a run that will not stop
  (`workspace.busy`) can still leave the children removed and the parent `removing`; `rm` again finishes it.
- The children are listed before the parent's removal begins. A child delegated between the check and the removal is not
  removed with it.
- The desktop removal dialog does not send `with_children` yet, so it meets only the `workspace.has_children` refusal.
