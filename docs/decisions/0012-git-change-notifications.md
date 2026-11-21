# 0012: Git change notifications

Status: accepted, 2026-11-21.

## Context

A client that shows status must learn when it changes. The change can come
from a file saved in an editor, an agent writing files, a commit made in a
terminal, a fetch, or another client. The current product polls, which costs
CPU on every open workspace and still lags. The service runs on Windows, Linux
and macOS, and later headless on a server with many workspaces.

## Decision

### A stream per workspace, fed by the operating system

- `git.watch { workspace }` is a stream method. Its events say what changed,
  never what the new state is. The client then asks `git.status`, `git.log` or
  `git.branches` again.
- The event carries `kinds`, a non-empty set of:
  - `worktree`: a file under the working tree;
  - `index`: `.git/index`;
  - `head`: `HEAD`, or the state files of a merge, rebase or cherry-pick;
  - `refs`: the `refs/` directory, `packed-refs`, or `FETCH_HEAD`.

  A client that shows only status can skip a `refs` event, and a branch list
  can skip a `worktree` event.
- Change detection uses the operating system's notifications through the
  `notify` crate: `ReadDirectoryChangesW` on Windows, inotify on Linux,
  FSEvents on macOS. Nothing is polled while the tree is quiet.
- Events are debounced in the service. A burst is reported once, 200 ms after
  it goes quiet, and at most 1 s after it began, so a long build still sends a
  steady trickle rather than silence.

### One watcher per workspace, shared

- The service keeps at most one watcher per workspace path, counted by its
  subscribers. The first subscription starts the watcher and the last one
  stops it. An unwatched workspace costs nothing.
- The watcher covers the working tree recursively. It also covers the
  repository's git directory and, for a linked worktree, the common directory
  where refs live.
- Inside the git directory, only the entries listed above count. `objects/`,
  `logs/` and `*.lock` files are ignored, because a single commit writes many
  of them.
- Events under `.git` inside the working tree are classified as git-directory
  events, never as `worktree`.

### No feedback loop

- Every read the service makes runs with `GIT_OPTIONAL_LOCKS=0` (0010). A
  status refresh therefore never rewrites the index.
- Without that, each status call would raise an `index` event, which would
  trigger another status call.

### Degraded watching is reported, not hidden

- Linux gives one inotify watch per directory, under a per-user limit
  (`fs.inotify.max_user_watches`). A tree that exceeds it, or a file system
  without notifications such as some network mounts, cannot be watched fully.
- The watcher then keeps the git-directory watch, which is a handful of
  directories, and reports `degraded: true` with a reason. The stream's first
  event always says whether watching is full or degraded.
- A degraded client may poll status at its own pace, and knows that it must.
- If the git directory itself cannot be watched, the stream ends with
  `git.watch_unavailable`.

### What is deferred

- Skipping ignored directories (for example `node_modules`) when the watches
  are set up. This would cut inotify use, but needs per-directory watches and
  ignore evaluation. It is deferred until the degraded case shows up in real
  repositories.
- Watching from a client machine into a remote host. The execution host owns
  the watcher (0015). A client never watches a remote path itself.

## Consequences

- A new crate, `core-watch`, wraps `notify` with the debouncing and the
  classification above, so the server holds no platform code.
- Tests write files, commit and fetch in real repositories. They expect events
  of the right kinds within a bounded wait, and none after the tree goes quiet.
