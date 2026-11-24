# 0055: Delegation and bring-back

Status: accepted, 2026-11-21.

## Context

In the old app an agent delegated by running
`orca orchestration delegate --prompt ... [--writes]`; the request was
filed, a child session started, and a finished child's branch was merged
into the parent's worktree with `git merge --no-ff` (AG-15, AG-31). The
owner's placement rules: each child gets its own worktree by default; the
current worktree only for read-only work; two sessions never write in one
worktree at once; parallel children always get their own worktree.

## Decision

- **`agent.delegate`** (`massorca agent delegate`) starts a child session
  for a parent session. It is called by a person, or by the parent agent
  itself: inside a run, `--from` defaults to `MASSORCA_AGENT_SESSION` and
  the call must then carry the matching `MASSORCA_AGENT_TOKEN`, so an agent
  can only delegate from its own session. Params: `parent`, `prompt`
  (required), `kind` (default: the parent's), `name`, `read_only`,
  `request_id`.
- **Placement.**
  - A writing child (the default) gets a **new child workspace**: a worktree
    in the parent's project, created through the ordinary workspace create
    (0008) with base = the parent workspace's branch. Its name is `name`, or
    `<parent workspace>-<n>` with the first free `n`; its branch is the
    workspace name. The child starts from the parent's last commit:
    uncommitted changes in the parent are not carried over, and the result
    says so.
  - A `read_only` child runs in the parent's workspace; the one-writer rule
    (0051) does not count it.
  - A parent in a folder workspace (no git) can delegate only `read_only`
    children in Phase 4; a writing child is refused with
    `agent.delegate_needs_git`, because nothing could keep the two writers
    apart.
- **Depth.** A child cannot delegate by default. The limit is `agents.delegation_depth`, default 1 (range 1 to 5), read on each call; see 0070 for the default and its source.
  The limit becomes a setting when someone needs deeper trees.
- **The tree.** A child's `parent_id` links it to its parent.
  `agent.tree <session>` returns the root and its children with their
  execution, activity and result, so one call shows the whole delegation.
  Children appear in `agent.watch` like any session.
- **Bring-back: `agent.bring_back`** (`massorca agent bring-back <child>`),
  run by a person or the parent agent, never automatically in Phase 4.
  1. The child must be `done` or `exited`; otherwise `agent.busy`.
  2. For a child with its own workspace, unless `--no-merge`: the parent
     must not be `working` and its worktree must be clean, else
     `agent.parent_busy` / `agent.parent_dirty`. Then, in the parent's
     worktree, `git merge --no-ff --no-edit <child branch>`. If the merge
     stops on a conflict it is aborted (`git merge --abort`), the parent is
     left as it was, and the call fails with `agent.merge_conflict` listing
     the conflicting paths. Nothing is ever pushed.
  3. Unless `--no-prompt`, and if the parent run is live, the parent
     receives a prompt (0054 rules, `force` off) that names the child, its
     result text and the merge commit. If the parent cannot take it, the
     bring-back still succeeds and says the prompt was not delivered.
  4. The child is marked `brought_back_at_ms`; a second bring-back is
     refused with `agent.already_brought_back`.
  The child workspace is kept; removing it is an ordinary
  `workspace remove`, so its branch and history stay inspectable.

## Alternatives

- **Automatic bring-back on `done`** (the old "auto access" path). It needs
  access modes and notifications that are not built; an explicit command is
  safer and scriptable, and an automation can call it later.
- **Rebase or squash instead of a merge commit.** A `--no-ff` merge keeps
  the child's history visible and is what the owner already uses.
- **Children in the parent's worktree by default.** Breaks the one-writer
  rule.

## Consequences

- Delegation, status and bring-back are all reachable from the CLI, so the
  M3 demo and the tests drive them without any UI.
- Routing by usage or a classifier (AUTO-20) is not built; the caller picks
  the kind.
