# 0103: Git file content

Status: accepted, 2026-11-25.

## Context

GIT-06 (file diffs for the unstaged, staged and HEAD comparisons) left one part
open: image previews. A Changes view draws a changed image only from the bytes of
each side. `git.diff_file` returns the binary note and the two sizes (decision
0011), and nothing more. pv96 reads the sides for its preview from the index, from
HEAD and from the working file. This decision takes the behaviour, not the code.

## Decision

`git.file_content` returns one file's bytes on one side of a comparison. Its
protocol constant sits beside `git.diff_file`.

- Params: `workspace`, `path`, `side`, and an optional `max_bytes`. The side is
  `worktree` (the file on disk), `index` (the index entry), or `revision` with a
  `rev` (a commit, branch or tag).
- Result: `path`, `size`, `too_large`, `content_base64`, and `mime`.
  - `size` is the byte count of that side. It is absent when the side has no file
    at the path, and then no bytes are sent either.
  - `too_large` is set when the file is over the cap. Its `size` is still known.
    No bytes are sent and nothing is cut short.
  - `content_base64` carries the bytes. It is absent when there are none.
  - `mime` is set for a previewable image, guessed from the extension: png, jpg,
    jpeg, gif, webp, bmp, ico and svg. It is absent for any other file. PDF is not
    previewed, by the owner's decision for now.
- The cap is 4 MiB by default and at most 4 MiB, the same limit as a patch. A
  larger value is clamped. The index and revision sides ask `cat-file -s` before
  they read a blob. The working file is measured before it is read. If it grows
  past the cap while it is read, the size reported is the size it has then.

### The request path

- The path is spelled once, before any side reads it. On Windows a backslash
  separates parts too. Empty and `.` parts are dropped, so `./README.md`,
  `docs//guide.md` and `docs\guide.md` name the same file on every side.
- A rooted path, a drive prefix, a verbatim prefix (`\\?\`) or a `..` part is
  refused with `git.path_outside_worktree`.
- A `.git` part, in any case, is refused with `request.invalid_params`. `.git/config`
  can hold a remote's credentials, and no workspace file is there.
- A path that names a directory or a submodule is refused with
  `git.path_is_directory`.

### The working-tree side

- The path is canonicalised and must lie inside the canonical workspace root. The
  two are compared without the verbatim prefix that Windows adds to a long path,
  so a path longer than 260 characters inside the workspace is read.
- The location must not lie inside the repository's git folders: this worktree's
  own folder (`git rev-parse --absolute-git-dir`) and the common folder that linked
  worktrees share (`--git-common-dir`). A repository whose folder has another name,
  inside the workspace, is refused too. A `.git` part anywhere in the location
  refuses a link that leads to git's own file.
- A symlink or a junction is followed only to a target still inside the workspace.
  A target outside it is refused with `git.path_outside_worktree`.
- The file is opened without waiting (`O_NONBLOCK` on Unix). Its open handle must be
  a regular file: a FIFO, a device or a socket is refused with
  `request.invalid_params`, and a directory with `git.path_is_directory`.
- After the open, the location is checked again before any byte is read. On Windows
  the check uses the handle's own final path (`GetFinalPathNameByHandleW`), which
  names the file that was opened. On Unix it re-resolves the path and compares the
  device and inode of the handle with those of the file the path names now.
- A read that cannot be completed reports `git.failed` with the workspace-relative
  path the caller asked for. No host path reaches the client.

### The index and revision sides

- They come from git's object store. `ls-files -s` and `ls-tree` run with literal
  pathspecs, then `cat-file` reads the blob. The `ls-files` output is capped at
  64 KiB: the first record of a path decides whether it is a file or a directory.
- A revision that does not name a commit is refused with `git.revision_not_found`.
  A revision that starts with a dash is refused the same way without running git,
  so it can never be read as an option.
- A path that is unmerged in the index is refused with `git.path_conflicted`.

### Other rules

- The method is a read, like `git.diff_file`. A caller without `workspace`
  permission runs git through the untrusted runner (decision 0078).
- The CLI is `massorca git show-file <PATH> -w WORKSPACE [--index | --rev REV]
  [--out FILE]`, shaped like `git patch`. `--out` writes the decoded bytes. With
  no bytes to write it fails with `git.no_bytes` and writes nothing.

## Alternatives considered

- **Fall back to HEAD on the index side**, as pv96 does when the index has no
  entry. Rejected, and kept rejected by the owner. After a staged deletion, an
  untracked file would then show HEAD's content as its old side and look modified,
  while git lists it as added. A client that wants HEAD asks for `side: revision`
  with `rev: HEAD`.
- **Never follow a symlink, or always follow it.** Never following would show a
  link's target text, which is what a diff does for an untracked link (decision
  0011), but it would give no preview of a linked image. Always following would let
  a link leave the workspace. Following only inside the workspace keeps both.
- **Encode the bytes in `git-core`.** Rejected. The protocol and the server own
  base64 for the other byte fields, so `git-core` returns raw bytes.
- **A third-party crate for file identity or the final path.** Rejected. `libc` and
  `windows-sys` are already workspace dependencies; core-git only enables a feature
  of the second one.

## Consequences

- The working-tree side reads any file inside the workspace, git-ignored files
  included, such as a `.env`. `git.diff_file` shows only untracked files that git
  does not ignore. The owner decided that this is allowed, because pairing already
  gives full access. Limiting the side to changed or untracked paths remains an
  option.
- On Unix a window remains. Between the open and the identity check, an attacker
  with write access to the workspace who swaps a folder for a link out of it, and
  back again, can make the check accept an outside file. This takes two swaps within
  a few system calls. Windows has no such window, because it checks the handle's own
  final path. Closing the Unix window needs `openat2` with `RESOLVE_BENEATH`, or a
  component-by-component open with `O_NOFOLLOW`; neither is in the workspace today.
- Each worktree read starts one `git rev-parse` for the git folders.
- A client must draw an SVG as an image (an `img` element or a data URL), never
  inline it as markup, because SVG can carry script.
