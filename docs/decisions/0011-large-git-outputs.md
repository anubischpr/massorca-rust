# 0011: Large git outputs

Status: accepted, 2026-11-21.

## Context

Diffs and logs can be arbitrarily large: a generated file, a vendored
dependency, years of history. A local frame is capped at 8 MiB (0002), and a
reply that would exceed it fails with `response.too_large`. The terminals work
adds binary stream frames, but they are not on `main` yet. A client also rarely
needs everything at once: a list shows file names, and one diff is opened at a
time.

## Decision

Every `git/1` reply is bounded by construction, so it always fits in one frame.
No reply streams in `git/1`.

### Status

- A status reply lists at most `limit` entries. The default is 1,000 and the
  maximum is 10,000.
- `truncated: true` says the cap was hit. The totals still count every entry,
  so a client can say "1,000 of 48,210 shown".

### Diffs come in two steps

- `git.diff` returns only the file list, with no patch text. Each entry has:
  - the path, the old path for a rename, and the change kind;
  - the added and removed line counts;
  - `binary: true` when git's numstat prints `-` for both counts.

  The list is capped like status.
- `git.diff_file` returns the patch of one path, capped at `max_bytes`. The
  default is 1 MiB and the maximum is 4 MiB.
  - When the patch is longer, the text is cut at the last line break before
    the cap and `truncated: true` is set. The size past the cap is not
    known, because git is stopped there.
  - A binary file returns no text, only its old and new sizes from
    `git cat-file -s`, so a client can show "binary, 2.1 MiB → 2.3 MiB".
- The modes are the same for both methods:
  - `worktree`: the index against the working tree;
  - `staged`: HEAD against the index;
  - `head`: HEAD against the working tree;
  - `range`: two revisions, `from..to`.
- An untracked file has no index entry, so `git.diff_file` shows it as
  wholly added, read from the working tree without following a symlink.
- Diffs always run with `--no-ext-diff --no-textconv --no-color`. A user's
  external diff tool or text converter never runs, because it can hang or
  print what git does not control.

### History is paged

- `git.log` takes a revision, a page size and an optional cursor. The default
  page is 50 commits and the maximum is 500.
- The first page resolves the revision to a commit id. The cursor carries that
  id and an offset, so later pages walk the same history even if the branch
  moves in between.
- The reply carries `next_cursor` when another page exists.
- Each commit carries its id, parents, author and committer, both times, and
  the subject. The full message is cut at 4 KiB per commit, so a page of 500
  commits stays far below a frame.

### Process output is capped too

- `core-git` stops reading a child's output once a per-call byte budget is
  reached, and kills the process tree. That cap sits above every reply cap, so
  a runaway command cannot fill the service's memory before the reply is cut.
- A read that hits the process cap reports `truncated`. It does not fail.

## Alternatives considered

- **Streaming whole patches over the event stream.** This would push the
  burden of reassembly and back-pressure onto every client. The per-file step
  covers the user-facing need. A streamed export (for example "save the patch
  of this range") can arrive later as its own method, on binary frames, once
  they are on `main`.
- **Server-side caching of diff results keyed by git state.** The current
  product needs this because it polls. With change notifications (0012),
  clients ask again only after a change. Caching is deferred until a
  measurement shows a need.

## Consequences

- Each reply's worst-case size can be computed and is tested against the
  frame limit.
- A client opens a diff in two round trips, and the first round trip stays
  cheap even for a huge change set.
