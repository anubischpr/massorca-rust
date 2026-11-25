# 0114: Repository identity from git remotes

Status: accepted, 2026-11-25.

## Context

Inventory row SCM-03 (`docs/inventory/providers-and-trackers.md`) asks that a repository is matched to its host and owner
from its git remotes, fork-aware. The service could fetch, pull and push, but it did not read what a remote names. The
owner parser for remote URLs is on other branches (commit `080c3c17`, `feat(core-git): read the owner of a remote URL
and of the primary remote`, on `origin/feat/core-batch-4`), not on `origin/main`. Decision 0010 puts provider actions
(pull requests, CI) in a later phase, so this decision covers the identity only.

## Decision

- D1 Identity per network remote. `git.remotes` (capability `git-remote-identity/1`) returns, for each network remote in
  git's order, `remote`, `host`, `provider`, `owner`, `repo`, `web_url` and `primary`, with `fork_of` when it is a fork.
  A remote whose URL is a local path, a `file://` URL, or has no owner and repository is not listed.
- D2 Parsing. https, http, ssh, git, git+ssh and ssh+git URLs, with an optional `user@` and port, and scp-style
  `[user@]host:path`, parse to one identity. The host is lowercased and never carries a user name or a port. The
  repository is the last path segment without `.git`; the owner is the rest of the path, so a GitLab group path such as
  `group/sub` is one owner. Azure DevOps paths (`org/project/_git/repo`, `v3/org/project/repo` over ssh,
  `org.visualstudio.com/project/_git/repo`) give the organization as the owner and the project for the web address.
- D3 Provider from the host. `github.com`; `gitlab.com` and any host containing `gitlab`; `bitbucket.org`;
  `dev.azure.com`, `ssh.dev.azure.com` and `*.visualstudio.com`; `codeberg.org` and any host containing `gitea`. Any
  other host, including a self-hosted one, is `unknown`, with its host, owner and repository still filled.
- D4 Web address. Given only for the known layouts: `https://{host}/{owner}/{repo}` for GitHub, GitLab, Bitbucket and
  Gitea, keeping the port only for http and https remotes (an ssh port is not the web port), and the Azure DevOps web
  layout. An unknown host has none, because a web address is not guessed. It never carries a user name or password.
- D5 Primary. `origin` is primary when it is a network remote; otherwise the first network remote is.
- D6 Fork. `origin` carries `fork_of` = `owner/repo` of `upstream` when both name the same repository on the same host
  under different owners. Owner and repository names compare without case.
- D7 Scope. `git.remotes` takes exactly one of a workspace or a project, and reads the repository with the same caller
  rules as `git.status` (decision 0078). It calls no provider API.

## Consequences

- The parser is one module, `crates/core-git/src/remote_identity.rs`. Its base is commit `080c3c17`; when that branch
  lands, its `remote_owner` module and this one must be reconciled into this module, not kept side by side.
- `Project.remote_owner` (on that branch) is not part of this change. A later change can read the owner through the
  same parser.
- Provider actions (pull requests, CI, issues) keep waiting for the phase decision 0010 names.
