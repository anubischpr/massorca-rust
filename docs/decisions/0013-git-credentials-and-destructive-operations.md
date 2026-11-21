# 0013: Git credentials, prompts and destructive operations

Status: accepted, 2026-11-21.

## Context

Fetch, pull and push talk to remotes. A remote can ask for:

- a username and password;
- an SSH key passphrase;
- a host key confirmation;
- a credential manager's sign-in window.

The service has no terminal and may run headless. A prompt that waits for an
answer blocks the operation, and on a server it blocks every client.
Credentials must never be copied into the service's logs, replies or events.

Discarding changes, amending, deleting a branch and pushing can all destroy a
user's work. 0008 set the rule for workspace removal: refuse by default, name
exactly what would be lost, and proceed only under an explicit waiver. The same
rule applies here.

## Decision

### Never prompt

Every git child runs with:

- `GIT_TERMINAL_PROMPT=0`, already set in Phase 1;
- `GIT_ASKPASS=` and `SSH_ASKPASS=`. Git treats an empty askpass as "no
  helper", and an empty value also skips `core.askPass`;
- `SSH_ASKPASS_REQUIRE=never`, which OpenSSH 8.4 and later honour;
- `GCM_INTERACTIVE=never`, so Git Credential Manager never opens a sign-in
  window;
- stdin closed.

On Unix, a service the CLI started is detached from any terminal. A service
started in the foreground with `service run` keeps its terminal, and a
user-configured SSH command or credential helper that opens `/dev/tty` itself
could still ask there. The operation timeout bounds that case. Putting each
child in a session of its own needs `unsafe` (`setsid`) and is deferred.

SSH runs in batch mode:

- When `GIT_SSH`, `GIT_SSH_COMMAND` and the repository's `core.sshCommand` are
  all unset, the child gets `GIT_SSH_COMMAND="ssh -o BatchMode=yes"`.
- A user-configured SSH command is used as is, and the operation timeout still
  bounds it.

Credentials that the user's git setup already supplies keep working:

- a credential helper or store;
- an unlocked ssh-agent;
- a token in the remote URL that the user configured.

The service never reads, stores or forwards a credential.

Remote operations have a timeout of 5 minutes, which the caller can lower.
The timeout kills the process tree. Cancelling a running git command when its
client disconnects is not built yet: the command runs to completion or to its
timeout.

### Typed outcomes, not text

- Fetch, pull and push return a typed result.
- A failure that git's text identifies becomes a typed code:
  - `git.auth_required` covers terminal prompts disabled, could not read a
    username, authentication failed, and permission denied for a public key;
  - `git.host_key_unknown`;
  - `git.remote_unreachable` covers DNS, connection and timeout failures;
  - `git.remote_not_found`.
- The CLI tells the user to authenticate in a terminal, for example by running
  `git fetch` there once, and to retry.

### Nothing secret leaves the child

- Before stderr appears in a reply, an error or a log line, every URL in it
  has its user-information part replaced with `***`. The text that is
  redacted includes `https://user:token@host/...`.
- The redaction has a test.
- `GIT_TRACE`, `GIT_TRACE_CURL`, `GIT_TRACE_PACKET` and `GIT_CURL_VERBOSE` are
  removed from the child environment, so a variable inherited by the service
  cannot make git print headers.

### Refusals and waivers

Each operation below refuses by default. A refusal names what would be lost.
A waiver is a separate boolean flag, so a client cannot waive more than it
names.

- **`git.discard`**
  - The default restores each named tracked path from the index, losing only
    its unstaged edits.
  - An untracked path is refused with `git.discard_untracked` unless
    `delete_untracked` is set.
  - A path with staged changes keeps them unless `include_staged` is set. With
    the flag, the path is restored from HEAD, and a newly added file is
    removed from the index and deleted, which also needs `delete_untracked`.
  - A conflicted path is refused with `git.path_conflicted`.
  - A directory path is refused with `git.path_is_directory`. Discard works on
    the files that status lists, never on a subtree.
  - Ignored files are never touched.
  - `all: true` stands for every file status lists, with the same waivers.
  - `dry_run: true` returns the plan without acting.
- **`git.commit` with `amend: true`**
  - Refused with `git.commit_published` when HEAD is reachable from any
    remote-tracking ref, unless `allow_published` is set.
  - Refused with `git.no_commits` before the first commit.
- **`git.branch_delete`**
  - Deletes safely, the way `git branch -d` does.
  - A branch that is not merged is refused with `branch.unmerged` unless
    `force` is set.
  - These are refused always, with no waiver:
    - a branch checked out in any worktree: `branch.checked_out`;
    - a branch that a workspace was created on: `branch.in_use`. Removing the
      workspace (0008) is the path that deletes such a branch.
- **`git.push`**
  - Never forces.
  - The only override is `force_with_lease`. It names the commit the remote
    branch must still point at, and the service always sends that commit
    explicitly, never a bare `--force-with-lease`.
  - A non-fast-forward rejection is a typed result (`rejected`), not an error.
- **`git.pull`**
  - The mode is explicit:
    - `ff_only` is the default: a pull that would need a merge returns
      `diverged` and changes nothing;
    - `merge` and `rebase` must be asked for.
  - A pull that stops in conflicts returns `conflicts` with the paths.
    `git.abort` undoes it.
- **`git.branch_switch`**
  - Git's own check is kept. It refuses when local changes would be
    overwritten, which becomes `git.checkout_would_overwrite` with the paths.
    Changes that do not conflict move with the switch, as they do in git.

## Consequences

- A headless service cannot hang on a credential prompt. The worst case is the
  operation timeout, and the typed result tells the client why.
- A destructive operation needs both a named path or branch and a named
  waiver. A client bug that sends the wrong flag loses at most what that flag
  names.
