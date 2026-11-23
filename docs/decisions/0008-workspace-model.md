# 0008: Projects and workspaces in Phase 1

Status: accepted, 2026-11-20.

## Context

`docs/inventory/workspaces.md` describes projects (a git repository or a plain
folder, bound to one host) and workspaces (a git worktree with a branch, or an
existing folder). Phase 1 must create, list, show and remove both kinds from
the CLI. Much of the inventory (setup scripts, generated names, status board,
lineage) comes later; the model must not block it.

## Decision

- **Project.** `{id, kind: git | folder, path, name, workspaces_root,
  added_at}`. `massorca project add <path>` registers it; a git repository is
  detected with `git rev-parse --show-toplevel`, and a linked worktree registers
  its main repository (found with `--git-common-dir`). `workspaces_root` defaults
  to `<user home>/massorca/workspaces/<project name>` and is a project property,
  so a test profile never writes into the real home.
- **Workspace.** `{id, project_id, kind: worktree | folder, path, name,
  branch?, base_ref?, created_branch, state: ready | removing, created_at}`.
  Names are unique within a project.
- **Git is the truth for which worktrees exist.** The store keeps identity and
  metadata. `show` and `list` report a workspace whose directory is gone as
  `missing` instead of hiding it.
- **Lock and prune state.** `show`, `list` and unregistered worktrees carry git's
  optional `locked` (its reason, `""` when none) and `prunable` (git's reason)
  fields; `prunable` needs Git 2.31 and stays absent on older git, and `missing`
  still comes from the directory.
- **Create a worktree.** Branch defaults to the cleaned name, behind the branch
  prefix (see Names). Base defaults to the repository's default branch: what
  `origin/HEAD` points at, else
  `origin/main`, `origin/master`, `main`, `master`, and `HEAD` only when none
  exists (GIT-13). A named base is resolved in the namespace it implies, so a
  tag of the same name never wins: a name with a `/` is a remote-tracking
  branch first, then a local branch; a plain name is a local branch; a full
  `refs/` name or a commit id passes through. The workspace records the name as
  given (`origin/main`). An existing local branch is used only with
  `--existing-branch`. Path is `<workspaces_root>/<name>` unless `--path` is
  given. Create carries an optional client request id; repeating the same id
  returns the first result instead of creating twice. A git project may set
  its own base (`massorca project set-base`), which sits between an explicit
  base and the repository default: a create that names no base starts from it,
  and clearing it restores the default. A folder project has none.
- **Refresh the base.** With `refresh_base` (`--refresh-base`), a create of a
  new branch first fetches the base's branch from its remote when the base
  resolves to a remote-tracking branch: the named base, else the project base,
  else the default branch. Only that branch is fetched, within 30 seconds, and
  nothing is pruned; the network rules, prompts and credentials are those of
  decision 0013. A failed or slow fetch never fails the create: it starts from
  the local copy, and the service log records a warning. A local branch, tag or
  commit base is not fetched, and the option is refused for an existing branch
  and for a folder project.
- **Interrupted create.** A create that stops after `worktree add` and before
  its row is stored leaves a worktree that git registers and no row records.
  The same create (same project, name, path and branch) adopts it: it stores
  the row and says the branch was not created by this request, so removal keeps
  the branch. A registered worktree that another row owns, that is on another
  branch, or that is the main worktree is never adopted and the path is taken.
  The adopted worktree's base is not verified.
- **Create a folder workspace.** Registers an existing directory (the project
  path by default). No files are created.
- **Remove.** The workspace row is first set to `removing`, then git runs
  `worktree remove` (with `--force` only when the user passes `--force`), then
  the row is deleted. A dirty or locked worktree is refused with a typed code.
  Git before 2.31 prints no `locked` line, so the lock is read from git's
  `locked` file in the worktree's admin record, which carries the same reason.
  The main worktree is never removed. A branch the workspace created is
  deleted with `git branch -d` (only if git proves it merged); otherwise it is
  kept and the result says so. A folder workspace is forgotten; its directory
  is never deleted. A failure before git acts returns the row to `ready`; a
  failure after git has removed the worktree leaves it `removing`, and `rm` again
  finishes the job.
- **Runs in a workspace being removed.** Once the row is `removing`, the
  service stops every agent run in the workspace and then every live terminal
  (a terminal that ignores the polite kill is killed by force), before git
  acts. No terminal or agent run can start in a `removing` workspace
  (`workspace.not_found`, "being removed"). A run that still does not stop
  refuses the removal with `workspace.busy`; the row stays `removing`, and `rm`
  again retries.
- **Changes a removal would lose.** Uncommitted and untracked changes in a
  worktree refuse with `workspace.dirty` unless `--force` is given. Ignored
  files are counted apart, because `git status` does not list them and
  `git worktree remove` deletes them; they refuse with
  `workspace.ignored_files`, and only `--discard-ignored` removes them. `--force`
  does not cover ignored files. A refusal returns the row to `ready`.
- **Orphan directories.** A directory that git does not recognise as its
  worktree refuses with `workspace.orphan`: a directory git never registered, one
  inside another repository, or a registered worktree whose `.git` file is gone.
  Git recognises a worktree only when it reports the directory as its own top.
  `--force` does not cover an orphan. `--forget-orphan` forgets the row and keeps
  the directory, which is never deleted; git keeps its record of the worktree
  until git prunes it, and the result says the directory was kept. This replaces
  forgetting an unregistered directory silently.
- **Unregistered worktrees.** `workspace list` compares git's worktrees under each git
  project's workspaces root with the rows. A worktree with no row is reported as
  `unregistered` (path, branch) and is never removed. It exists after a create that
  stopped between `worktree add` and the row insert. Listing takes the mutation
  lock, so a create in progress is not reported.
- **Project removal.** `project.remove` is refused with `project.has_workspaces`
  while the project has workspaces. With `forget_workspaces` it deletes the project
  row and every workspace row in one transaction, and publishes `project.removed`
  and one `workspace.removed` per row. No file, folder, worktree or branch is
  touched, and git does not run. It is refused with `workspace.busy` while a
  workspace has a live terminal or agent run, or is `removing`, because forgetting
  the records would orphan those runs. Ended agent sessions of the workspaces are
  deleted with their rows (`ON DELETE CASCADE`), as `workspace.remove` already
  does; their run folders and exited terminal records stay in the profile.
- **Selectors.** An id or an exact name selects a workspace or project. A selector
  that is a path must be absolute; a relative one is refused with
  `request.invalid_params`, because it would resolve against the service's own
  directory.
- **Names.** A typed workspace name is cleaned first: accents fold (`é` to `e`,
  `ß` to `ss`, `æ` to `ae`, `ø` to `o`, `œ` to `oe`, `ł` to `l`, `đ` to `d`),
  every other character outside `[A-Za-z0-9._-]` becomes `-`, runs of `-` and of
  `.` collapse, and leading or trailing `.` and `-` go. The cleaned name is the
  stored name, the folder and the branch leaf. The reference app keeps accented
  letters; this app keeps its ASCII set. A name is refused only when cleaning
  cannot make it valid: nothing left, longer than 64, or a Windows device name
  (`CON`, `NUL`, `COM1`, ..., with or without an extension, in any case). A branch
  git refuses (`a.lock`, an explicit bad `--branch`) is refused as before. A
  project name is not cleaned: it refuses a colon, a trailing dot or space, and
  `.` or `..`. A default workspaces root and a project's root must be absolute.
- **Branch prefix.** The `workspaces.branch_prefix` setting picks the prefix of a
  branch that a create makes itself: `none` (the default), `git-username`, or
  `custom` with the text of `workspaces.branch_prefix_custom`. A custom prefix is
  normalised (surrounding slashes and spaces go, runs of `/` collapse), and one
  git refuses fails the create with `config.invalid_value`. The git username is
  `github.user`, then `user.username`, else `user.name` when it is a plain token
  (a letter or digit first, then letters, digits, `.`, `_` or `-`); when none
  is usable there is no prefix. The prefix never applies to a branch the request
  names or to an existing branch. A delegated child keeps its name `<parent>-<n>`
  and its branch (0055).
- **Generated names.** A blank or omitted name asks for a generated one: a marine
  creature in lowercase (`nautilus`, `seahorse`, ...), chosen at random from the
  creatures that no workspace of the project, no local branch (under its prefix)
  and no directory under the workspaces root takes. Once every creature is taken
  the next tier is used (`nautilus-2`).
- **Errors** are typed codes (`workspace.dirty`, `workspace.locked`,
  `workspace.not_found`, `workspace.name_taken`, `project.not_git`, ...), never
  classified from git's message text. One exception: `git rev-parse
  --show-toplevel` exits 128 both outside every repository and on other
  failures (checked on Git 2.25.5, 2.43 and 2.49), so `Git::toplevel` matches
  "not a git repository" in the message. Every git call runs under `LC_ALL=C`
  (ADR 0007), which keeps that text English.

## Deferred (recorded so it is not forgotten)

Retirement of a generated name after its workspace is removed (a removed name
can be issued again today), nested layout options, setup scripts, terminal and
agent guards on removal, squash-merge detection, id aliases after a folder
rename, remote hosts. Each lands with its phase and its own tests.

## Consequences

- Removal is resumable: running `rm` again on a `removing` workspace finishes
  the job.
- Adding the deferred behaviour adds columns and options, not a new model.
