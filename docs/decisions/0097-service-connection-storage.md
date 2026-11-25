# 0097: Service connection storage

Status: accepted, 2026-11-24. Covers the service and CLI part of SKL-35; the desktop Services section follows.

## Context

pv96 connects hosted-account services from its Marketplace: GitLab, Bitbucket, Azure DevOps and Gitea, with Linear and
Jira shown alongside. Its tokens sit in a sealed store of their own. MassOrca already has a vault (0071) with one master
key per profile, sealed rows, write-only methods and no value in any log, error or `Debug` output. A second store would
need its own key, backups and redaction.

## Decision

- **Vault entry kinds.** A connection is up to three sealed rows of the existing `secrets` table under the scope
  `service:<id>`: `TOKEN`, `BASE_URL` and `EMAIL`. The associated data is the usual `massorca-secret/1`, row id, scope
  and name, so a row moved to another service or field does not open. No store migration is needed.
  - `secret.*` cannot name these rows: `service:` is not a scope those methods parse. `secret.list` and the count in
    `secret.status` leave them out, and no agent environment receives them.
  - `secret.reset_key` still refuses while a service row exists, because the key seals it.
  - A vault from before the key check accepts its key only when the service rows open too.
- **Write-only over RPC.** Capability `services/1`:
  - `marketplace.service.status` `{ service? }` (Read): id, name, state (`connected`, `not-connected`, `unknown`),
    source (`stored`, `external`, `none`), and the account email, base URL, update time, `glab` presence and detail when
    known. Never a token.
  - `marketplace.service.connect` `{ service, token, base_url?, email? }` and `marketplace.service.disconnect`
    `{ service }` need HostAdmin, the class `secret.set` uses, because both write a credential.
  - The connect params decode with the vault's redacted errors, and their `Debug` prints `[redacted]`.
- **Where it lives.** The connection is stored on the host that runs the service the client addresses (`--host`). That
  host owns the connection, as it owns its vault.
- **GitLab.** `glab` keeps its own sign-in, as in pv96. Connect runs `glab auth login --hostname <host> --stdin` through
  `massorca_process`, with the token written to stdin and the output discarded, because it may repeat the host or the
  token. A host with a port goes in `GITLAB_HOST` instead, since `--hostname` refuses one. Without a base URL the host
  is `gitlab.com`. Status runs `glab auth status`, and a missing `glab` says so. Disconnect is refused with
  `marketplace.service_unavailable`, as in pv96, because there is nothing of MassOrca's to delete.
- **Bitbucket, Azure DevOps and Gitea** store the token, and the base URL and, for Bitbucket, the email when given. A
  connect replaces every field; a field left out is cleared.
- **Linear and Jira** are status-only rows that report `unknown` until the providers-and-trackers rows connect them.
- **The CLI.** `massorca marketplace services list|connect|disconnect`. `connect` reads the token from stdin, a hidden
  prompt, or the variable `--token-env` names, never from an argument.

## Alternatives

- **A store of its own, like pv96's.** It would need a second key and its own redaction and backups.
- **Storing the GitLab token in the vault as well.** `glab` would still need it on disk in its own config, so the vault
  copy would only be a second place to rotate.

## Consequences

- MassOrca's own source-control calls can read a stored token from the vault later. No method returns one to a client.
- The vault key now protects service tokens too, so losing it locks them with the secrets.
- Whether MassOrca should sign `glab` out on disconnect is open; pv96 does not.
