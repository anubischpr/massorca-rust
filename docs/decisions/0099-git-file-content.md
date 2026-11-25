# 0099: Git file content

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
    jpeg, gif, webp, bmp, ico and svg. It is absent for any other file.
- The cap is 4 MiB by default and at most 4 MiB, the same limit as a patch. A
  larger value is clamped. The index and revision sides ask `cat-file -s` before
  they read a blob. The working file is measured before it is read.
- The working-tree side is read from disk under the workspace root:
  - A path that is absolute, has a drive prefix, or contains `..` is refused with
    `git.path_outside_worktree`, as the existing path guard refuses it.
  - The path is canonicalised, so a symlink is followed only to a target still
    inside the workspace. A symlink whose target lies outside is refused with
    `git.path_outside_worktree`.
  - A directory or a submodule is refused with `git.path_is_directory`.
  - A path inside git's own `.git` folder is refused with `request.invalid_params`.
    `.git/config` can hold a remote's credentials, and no workspace file is there.
- The index and revision sides come from git's object store. `ls-files -s` and
  `ls-tree` run with literal pathspecs, then `cat-file` reads the blob. A revision
  that does not name a commit is refused with `git.revision_not_found`. A revision
  that starts with a dash is refused the same way without running git, so it can
  never be read as an option. A path that is unmerged in the index is refused with
  `git.path_conflicted`.
- The method is a read, like `git.diff_file`. A caller without `workspace`
  permission runs git through the untrusted runner (decision 0078).
- The CLI is `massorca git show-file <PATH> -w WORKSPACE [--index | --rev REV]
  [--out FILE]`, shaped like `git patch`. `--out` writes the decoded bytes. With
  no bytes to write it fails with `git.no_bytes` and writes nothing.

## Alternatives considered

- **Fall back to HEAD on the index side**, as pv96 does when the index has no
  entry. Rejected. After a staged deletion, an untracked file would then show
  HEAD's content as its old side and look modified, while git lists it as added.
  A client that wants HEAD asks for `side: revision` with `rev: HEAD`.
- **Never follow a symlink, or always follow it.** Never following would show a
  link's target text, which is what a diff does for an untracked link (decision
  0011), but it would give no preview of a linked image. Always following would let
  a link leave the workspace. Following only inside the workspace keeps both.
- **Encode the bytes in `git-core`.** Rejected. The protocol and the server own
  base64 for the other byte fields, so `git-core` returns raw bytes.

## Consequences

- The working-tree side can read any file under the workspace, git-ignored files
  included, such as a `.env`. `git.diff_file` shows only untracked files that git
  does not ignore. Any caller with `read` permission can therefore read ignored
  files through this method. Limiting the side to changed or untracked paths is a
  possible follow-up.
- The containment check and the read are two calls. A symlink swapped in between
  could be followed. That needs write access to the workspace, which the caller
  does not have.
- A client must draw an SVG as an image (an `img` element or a data URL), never
  inline it as markup, because SVG can carry script.
- PDF is not in the list of previewable types, although pv96 previews it.
