# 0098: Account cleanup and one account per login

Status: accepted, 2026-11-24.

## Context

The owner removed the Claude package, reinstalled it, imported the same login again, and the account list showed two
accounts, both labelled `account`, both signed in. Three causes:

- Removing a package deleted no account. `extension remove` runs the registered remove cleanups only with
  `delete_agent_homes` (decision 0062), and the agents feature registered a remove guard but no cleanup.
- Claude's identity was never captured. Its check was `claude auth status` with no `identity_pattern`, so a check
  never yielded an identity and the duplicate test (decision 0059, ACC-03) never ran.
- When the identity was known, a second import of the same login was refused ("activate that account instead"),
  which leaves the person with the old files.

## Decision

- **Removal cleanup.** The service registers the agents' remove cleanup next to the guard. It runs only when a removal
  asks for `delete_agent_homes`, after the package's files are deleted.
  - A cleanup is now called with a `RemovedPackage { id, agent_kinds }` instead of the bare id. The extensions read the
    kinds from the manifest before the files are deleted. A manifest that cannot be read gives no kinds, and the
    removal reports that their accounts were left: the lockfile's CLI records are not a safe guess. A first-party id
    the catalogue does not vouch for provided no kind, so it passes none.
  - A package outside `massorca.*` never cleans a kind the built-in catalogue declares; that kind is skipped and
    reported.
  - For each of those kinds that no other installed package provides, every account row of the kind is deleted, with
    its active mark and project choices. So are its directory under `<profile>/agents/accounts/<kind>/`, any account
    directory left without a row, the hidden folders an interrupted import or replace left (they may hold tokens), the
    `accounts/<kind>/` folder itself, and the kind's 0056 home with its hidden siblings. The deletions use the existing account-home helpers, which
    refuse a link instead of following it.
  - An account busy with a sign-in, a decision, a removal or a live run is kept. What was not deleted is one cleanup
    failure in the removal result, with the code of its first problem (`agent.busy` only for a busy account).
  - No sign-out runs. An account a MassOrca sign-in made keeps its token valid at the vendor until the person signs
    out there.
- **JSON checks.** A check gains two optional fields, validated as the kind's problems:
  - `identity_json`: top-level string fields of the check's JSON output, joined with ` / ` in order, those present
    only. Declared instead of `identity_pattern`, so a field nested deeper or a second `email` elsewhere is not read.
  - `signed_in_json`: a top-level field whose `false` means the login needs a sign-in, even on the signed-in exit.
- **Claude's identity.** Claude's check is `["auth", "status", "--json"]` with `signed_in_exit: 0`,
  `identity_json: ["email", "orgName"]` and `signed_in_json: "loggedIn"`. One email in two organisations is two logins.
  The previous app reads this command's `email`. Both copies of the declaration change together: the built-in one and
  the catalogue package's. Below `workspace` scope the identity stays masked.
- **One account per login.** A new account whose check reports the identity of an existing account of the kind updates
  that account instead of being refused. This covers `agent.accounts.import` and the end of `agent.accounts.add`.
  - Under the homes gate, each declared credential file the new account holds is written into the existing account's
    home: to a new owner-only sibling, renamed over the old file, a link refused. Nothing else in that home changes,
    since an agent home also holds transcripts, history and settings, and a declared file the new account lacks is
    kept. The new account's home and row are deleted, and the existing account's state and identity are refreshed.
  - The existing account keeps its id, label, origin, active mark and project choices. Nothing is signed out.
  - An import answers `imported` with the existing account and the new optional field `updated: <its id>`.
    `AccountsImported.updated` is omitted when absent, so an older peer ignores it. The CLI prints `updated account
    <id> with the imported files instead of adding one`.
  - When the existing account is busy, the new account is deleted as before: an import fails naming that account, and
    a sign-in is announced as `conflict`.
  - `agent.accounts.add` answers when its sign-in starts, before the check, so its result cannot say which account
    was updated. `AccountsAdded` gains no field. The update is announced through `agent.accounts_changed`: the new
    account, which is gone, and the updated one.

- **A crash during a replace.** The 0056 `import --replace` still swaps whole homes. The startup sweep renames an aside
  home (`.<name>.old-<id>`) back when its home is missing, and deletes it when its home exists. The import's refusal
  of such a leftover says it can be renamed back.

## Consequences

- `extension remove --delete-agent-homes` followed by a reinstall and an import gives exactly one account.
- Importing or signing in to the same login twice gives one account with the newest files.
- `Extensions::register_remove_cleanup` takes a closure over `&RemovedPackage`. A closure that ignores its argument
  is unchanged.
- The desktop's package removal sends `delete_agent_homes: false`, so it does not run this cleanup yet. Its confirm
  text says the package's connections are removed.
- A kind keeps its account history after the cleanup. A reinstalled kind with no account still refuses to fall back to
  the vendor's usual home (decision 0059).
