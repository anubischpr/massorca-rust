# 0071: Secret vault

Status: accepted, 2026-11-21. Needs an independent security review before the
code merges.

## Context

Agents and extensions need API keys and tokens. The owner's rule (design 8):
"values never go into a chat, a prompt or a log; sharing creates a link, not a
copy." The desktop already draws the secret request card (X-CH-07) and the
entry modal (X-VT-01). The service has no store behind them.

Each secret has a variable name, a value and an owning scope: a project, a team
or global. Another scope can link to it. Agents receive it as an environment
variable when they start. A session that is already running keeps the value it
started with (A30). The service often runs headless, on a server with no
desktop session and no keyring.

0001 binds an extension's secrets to the hosts they may be sent to, and the
host injects them, so extension code never sees a value.

## Decision

### Clients never read a value

No method returns a secret's value to any client: not the desktop, not a phone,
not the CLI.

- A value enters the service through `secret.set`.
- It leaves only into a process the service itself starts, as an environment
  variable:
  - **agent sessions:** the agents domain asks the vault for the secrets that
    apply to the session's project at launch;
  - **later, extension host imports** (0001).
- A grant is therefore the scope:
  - a project-scoped secret reaches agents started in that project;
  - a team-scoped secret reaches the team's bots;
  - a global secret reaches every agent on the host.

This rules out the most common leak, a client that asked for a value it should
not have. It also means the vault screen can never show a value, which the
design already requires (A31).

### Storage

**Ciphertext in the profile database.** It lives in a new `secrets` table, one
row per owned secret:

- id, scope, name;
- nonce and ciphertext;
- created, rotated and last-used times.

Links are rows in a `secret_links` table that point at the owning row. Names
and scopes are stored in clear, because the vault lists them without
unlocking.

**Cipher.** XChaCha20-Poly1305 with a random 24-byte nonce per write. The
associated data is `massorca-secret/1`, the row id, the scope and the name. A
ciphertext moved to another row or renamed therefore fails to open.

**The master key** is 32 random bytes, one per profile. Where it lives is
recorded at creation in `<profile>/vault/protection`, and later starts never
switch silently:

| Host | Key protection |
|---|---|
| Windows | DPAPI, current-user scope, with UI forbidden. The wrapped key is the file `<profile>/vault/key`. |
| Linux, Secret Service reachable and unlocked | The key is an item in the default collection, labelled `MassOrca vault <id>`, where `<id>` is 16 random bytes in hex kept in `<profile>/vault/id`. No item carries a path. Unlocking is never requested, because a prompt could hang the service. A Secret Service that is missing, locked or slower than 2 seconds counts as unavailable. |
| Linux without Secret Service, the headless case | An owner-only file `<profile>/vault/key`, mode 0600, in a 0700 directory. `secret.status` reports `protection: "file"`, and the CLI prints that the key sits on disk next to the data. |
| macOS | Keychain. This is not validated here, because no macOS machine is available. Until it is, macOS uses the file. |

On headless Linux the alternatives were to refuse to store secrets, or to ask
for a passphrase at every service start. Refusing makes the vault useless on
the server, which is where agents run. A passphrase blocks unattended
restarts. A file key protects only against copies of the database, such as
backups, and that limit is stated in `secret.status`.

`MASSORCA_VAULT_PROTECTION=file`, read at service start, forces the file key.
Test sandboxes use it, so they never write items into the user's real keyring.

**A key that will not open** (keyring reset, another user, a database copied
from another host) does not lead to data loss:

- the vault reports `locked`, with the typed code `secret.vault_locked`;
- reads and injection fail with that code;
- the ciphertext is kept, never dropped and never re-keyed.

Secrets with no readable `protection` file, such as a lost vault directory,
are locked the same way: a key is created only while the vault holds no
secret. One secret that does not open fails injection with
`secret.unreadable`, which names its scope and name; nothing is marked used.

The only ways out are restoring the key or `secret.remove` per name, which
needs `force: true`, because the value cannot be checked.

### Methods

Capability `secrets/1`. Every method that changes the vault needs HostAdmin.
Listing needs Read, because names are what the request card and the vault
screen show.

- **`secret.status`**: `{ protection, state: ready | locked, count }`.
- **`secret.list`** `{ scope? }`: names, scope, owner or link target, linked
  scopes, created, rotated and last-used times. Never a value or a length.
- **`secret.set`** `{ scope, name, value, replace? }`:
  - creates a secret;
  - with `replace: true`, rotates an owned one. An existing name without
    `replace` is refused with `secret.name_in_use`;
  - rotating or setting through a linked name is refused with `secret.linked`,
    which names the owning scope;
  - the value is at most 64 KiB of UTF-8.
- **`secret.link`** `{ scope, name, from_scope }`: links to the secret with the
  same name in `from_scope`. Design question 10 keeps the name, so a link never
  renames.
- **`secret.unlink`** `{ scope, name }`: asks nothing (A34).
- **`secret.remove`** `{ scope, name, include_links? }`:
  - removing an owned secret that has links is refused with `secret.has_links`,
    which lists the linked scopes;
  - `include_links: true` removes the secret and every link, the waiver
    discipline of 0008 and 0013.
- **Requests**, the card's backend:
  - `secret.request` `{ scope, name, reason, session? }`: an agent session, or
    the owner, asks for a name. Name, reason and title are untrusted text: the
    name is validated, the reason is plain text of at most 200 characters, and
    the session of at most 128, refused with `secret.invalid_session`. Reason,
    session and scope ids refuse control characters, Unicode format
    characters (Cf, bidi controls included) and U+2028/U+2029.
  - `secret.requests`: lists open requests.
  - `secret.decline` `{ request }`: declines one.
  - A request resolves when a matching `set` or `link` lands. Its states are
    the card's: pending, stored, linked, declined, failed.
  - Deleting a secret leaves the old request as history. The next request for
    that name opens a new one.

Names are 1 to 64 characters: letters, digits and `_`, not starting with a
digit (X-VT-01). Scopes are `global`, `project:<id>` and `team:<id>`. Team
scopes are accepted once Bot Teams has teams.

Events, ids only and never values:

- `secret.changed`, with id `<scope>/<name>`, after set, rotate, link, unlink
  and remove;
- `secret.requested` and `secret.request_changed`, with the request id.

### Redaction

**In memory.**
- Values live in a `SecretValue` type. It zeroes its buffer on drop.
- Its `Debug` and `Display` print `[redacted]`, and it does not implement
  `Serialize`.
- The only way to the bytes is one explicit method, which the injection path
  uses.

**On the wire.** The `secret.set` params are the one place a value travels.
- The connection layer does not log request params for any method.
- A test pins that a value sent through `secret.set` appears in no log line at
  any level.

**In errors.** No error message, error data or event includes a value. They
carry the name and scope only.

**In the CLI.** `massorca secret set <scope> <name>` reads the value from stdin
when stdin is not a terminal, or else from a no-echo prompt. It never reads it
from an argument or an environment variable: those show up in process listings
and shell history. The prompt takes one line: input still pending after Enter,
as from a multi-line paste, is discarded and refused with
`secret.paste_incomplete`; multi-line values go through stdin. `list` and `status` never print values, because the service
never has one to give.

**In agent output.** An agent can still echo a value into its own terminal or
transcript. Scrubbing known values from agent output is not built here. It is
a follow-up for the agents domain, which owns that output.

### CLI

```
massorca secret status
massorca secret list [--scope <scope>]
massorca secret set <scope> <name> [--replace]
massorca secret link <scope> <name> --from <scope>
massorca secret unlink <scope> <name>
massorca secret remove <scope> <name> [--include-links] [--force]
massorca secret requests
massorca secret decline <request>
```

## Alternatives

- **One OS credential entry per secret.**
  - Windows Credential Manager caps a blob at 2560 bytes.
  - Secret Service is absent on servers.
  - Every test profile would leave entries in the user's real keyring.
  - Only the 32-byte key goes to the OS store.
- **A `secret.get` with per-client grants.** It would let a client read a
  value. The design needs no client to see one, and every grant record would be
  a new thing to get wrong.
- **An encrypted file outside the database.** It would need its own atomic
  writes, its own backups and its own link integrity. The database already has
  transactions and is already backed up, and the backups then hold only
  ciphertext.

## Consequences

- The agents domain launches with `vault.environment_for(project)`. It is not
  wired here, because agent sessions are still on the phase-4 branches.
- Profile backups contain ciphertext only. A restore on another machine opens
  as `locked` until the key is restored.
- On a host whose key protection is `file`, anyone who can read the profile
  directory can read the secrets. `secret.status` says so.
- An independent security review runs before the vault code merges.
