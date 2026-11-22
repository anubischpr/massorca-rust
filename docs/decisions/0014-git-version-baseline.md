# 0014: Git version baseline for the operations surface

Status: accepted, 2026-11-21.

## Context

0007 fixed Git 2.25 as the oldest supported version. It is what Ubuntu 20.04
ships, and our glibc floor targets the same release. Users run whatever git is
on the host: Windows, a WSL distribution, or an SSH host, each possibly a
different version. CI already builds Git 2.25.5 from source and runs the
`core-git` and `core-workspace` tests against it.

## Decision

### Every command and option must exist in 2.25

| Need | Used | Avoided (version) |
|---|---|---|
| Status | `status --porcelain=v2 -z --branch` (2.11) | |
| Line counts | `diff --numstat -z` | |
| Path lists | arguments in chunks of at most 32 KiB, after `--` | `--pathspec-from-file` (2.26) |
| Literal paths | global `--literal-pathspecs` | |
| Discard | `checkout -- <paths>`, `checkout HEAD -- <paths>` | `restore` (2.23, experimental then) |
| Switch | `checkout <branch>`, `checkout -b <new> <start>` | `switch` (2.23, experimental then) |
| Unstage | `reset -q -- <paths>`, which also works on an unborn HEAD | `restore --staged` |
| Push result | `push --porcelain` | |
| Fetch result | exit status and ref comparison before and after | `fetch --porcelain` (2.41) |
| Ahead and behind | the `# branch.ab` header of status v2 | |
| Merge prediction | not offered in `git/1` | `merge-tree --write-tree` (2.38) |

### Newer options get a probe and a fallback

- An option newer than 2.25 is used only behind a capability check.
- The check probes once per git binary, and per execution host when there are
  several hosts (0015). It is cached for the service's lifetime.
- The older path must produce the same typed result.
- No newer option is needed in `git/1`. The rule is fixed now so that the
  first one arrives with its fallback and with a test on the 2.25 job.

### The 2.25 job runs the operations tests

- The CI job that builds Git 2.25.5 runs every `core-git` test, including the
  new operation modules. It also runs the server's git method tests.
- A test that needs a newer git does not silently skip. It names the version
  it needs and asserts the fallback's behaviour.

## Consequences

- Some outputs are parsed from older, more verbose forms. For example, fetch
  compares refs before and after instead of reading `--porcelain`.
- A user on Ubuntu 20.04 gets the same features as a user on the newest git.
