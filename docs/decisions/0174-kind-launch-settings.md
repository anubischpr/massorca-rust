# 0174: Standing launch settings of an agent kind

Status: accepted, 2026-11-26.

## Context

Inventory row AG-11 asks for per-agent Command, Arguments and Environment settings: a user replaces the binary or
wrapper an agent runs, adds standing arguments, and sets variables, for example to point an agent at another provider
or proxy. The pv96 reference keeps three settings per agent (`agentCmdOverrides`, `agentDefaultArgs`,
`agentDefaultEnv`) and applies them to terminal launches and structured chats alike; its environment record may hold
secrets, and a refusal never echoes a command value.

Before this decision a user kind file named its own program and arguments, and `agent.start` took per-start `args` and
`env`, but a built-in kind had no standing override, and the CLI sent no `env`.

## Decision

D1 What is kept. Per kind: `program` (replaces the declared program list), `args` (added after the kind's own launch
arguments and the mode and model flags, before a start's own `args` and the prompt) and `env` (set in the run's
environment under a start's own `env`). Any declared kind takes them, built in or from a kind file: one rule for both,
and a kind file can keep naming its own program.

D2 Where. The settings of a kind are one sealed row of the profile's vault, scope `agent-kind:<kind>`, name `SETTINGS`,
holding the whole record as JSON. An environment value may be a secret, so the record is sealed like a secret: the
vault's own encryption and key, no new store and no migration. No `secret.*` scope can name the row: secrets never list,
count, link or hand it to a run, and resetting the vault key is refused while any kind has settings
(`secret.vault_not_empty` with `agent_kinds` in its data). A locked vault refuses a change, and a start of a kind that
has settings, with `agent.secrets_unavailable`; a kind without settings never opens the vault. Clearing needs no key.

D3 Wire. Capability `agent-settings/1`:

- `agent.settings.get { kind }` answers `{ kind, program?, args?, env_names? }`: the variable names, never their values.
  It answers for a kind that is no longer declared too, so leftover settings can be seen and cleared.
- `agent.settings.set { kind, program?, args?, env?, remove_env? }` changes what it names and answers like `get`.
  `program` replaces the program and an empty one removes it; `args` replaces the list and an empty list removes it;
  `env` sets or replaces those names; `remove_env` removes names. Values are write-only, as a service token is. Settings
  left empty are removed, not stored. The kind must be declared.
- `agent.settings.clear { kind }` removes them: `{ kind, removed }`.

All three need host-admin scope: the settings choose what every run of the kind executes, and its environment may hold
secrets, as `secret.set` does. `set` params are decoded with redacted errors (a wrong-typed value is never repeated),
and `Debug` of the params and of the stored record lists names only.

D4 Checks on `set`. The kind id is `[a-z0-9-]{1,32}` and declared (`agent.kind_not_found`). The program is an absolute
path or a bare name, as the program search resolves kind programs, without control characters, and must resolve on the
service's host now (`agent.not_installed`). Arguments are at most 256, each at most 8 KiB without NUL. Variable names
are not empty and hold no `=` or control character; a `MASSORCA_*` name or the kind's credential home variable is
refused, as a vault secret of that name is. Values hold no NUL. A refusal names the field or the variable name, never a
value or the program (`request.invalid_params`, `agent.not_installed`), and changes nothing.

D5 Where they apply. The program replaces the declared one wherever the service runs the kind: terminal and chat
starts, resumes, forks, delegated children, sign-in, account checks and one-shot title runs. When the program does not
resolve any more, the start is refused with `agent.not_installed`, whose message never names it. The arguments and
variables apply to every terminal and chat run: starts, resumes, forks and delegated children. A resume takes them,
unlike a start's own `args` and `env`, which a session does not keep: they belong to the kind, not to the session.
Sign-in, account checks and one-shot runs take the program only.

D6 Precedence. A start's own values win: its `args` come after the standing ones (a CLI that reads an option twice
keeps the last), and its `env` replaces a standing variable of the same name (compared without case on Windows). The
standing values win over the kind's declaration. Standing variables are passed as the start's own `env` is, so they
follow its rules: the run's identity variables override them, and a run that receives vault secrets refuses a name that
loads code, redirects lookups or swaps an endpoint (`agent.secrets_unavailable`, decision 0071). Standing arguments, like
a start's, mark the run's permission mode as not confirmed.

D7 CLI. `massorca agent settings show|set|clear <KIND>`. `set` takes `--program PATH` or `--no-program`, `--arg ARG`
(repeatable; any `--arg` replaces the list) or `--no-args`, `--env KEY=VALUE` (repeatable), `--secret-env KEY` (the
value comes from stdin, or a hidden prompt, as `secret set` reads one, so it never sits in an argument) and
`--unset-env KEY` (repeatable). A relative program path is made absolute against the current directory. `agent start`
takes `--env KEY=VALUE` (repeatable) and sends it as `env`; it already sent `-- ARGS`.

## Consequences

- No existing type changes shape. The four params and result types are new, `AgentsConfig` gains `kind_settings`, and
  the vault gains its rows, a refusal for an invalid kind and one for a key reset while settings exist.
- `agent kinds` still reports a kind as not detected when only its standing program resolves; the start works.
- Follow-ups: a listing of every kind's settings for a settings screen, the standing program and environment for
  commit-message generation (AG-27) once it exists, the environment for sign-in and account checks, and the pv96 rule
  that a chat's program must end in `.exe/.com/.cmd/.bat` on Windows, which the program search already enforces through
  `PATHEXT` for bare names but not for an absolute path.

## Amendment 1, 2026-11-27: review findings

A review of the first implementation found four gaps; each rule below replaces the part of D3 to D7 it names.

A1 Standing variables follow the vault's boundary (amends D5 and D6). They reach a run only when its caller holds
host-admin scope, the caller that receives vault secrets (`SecretGrant::Provide`, decision 0071). A run started, resumed,
forked or delegated by any other caller gets its own `env` only, as if the kind had no standing variables; it still takes
the standing program and arguments, which are not secret. A run that receives at least one standing variable is marked
`holds_secrets`, as a run that receives a vault secret is, and the mark stays with its session: its scrollback and its
children are then guarded as for any run holding secrets. The record is still read for such a caller, because it holds
the program and arguments too, but its variables are dropped before the launch is planned.

A2 Standing arguments leave a resumed mode unconfirmed (amends D6). A resume adds the standing arguments after the mode's
own, so a resumed permission mode is recorded as not confirmed whenever the kind has standing arguments, exactly as a
start's own arguments or standing arguments leave a started mode unconfirmed. Before this, a resume could record a mode
as confirmed although a standing argument may override it.

A3 The CLI never repeats a variable's value (amends D7). A malformed `--env` of `agent start` or `agent settings set`
(no `=`, or no name before it) is refused with a usage message that does not repeat the text given, unlike clap's own
error for a value parser. The parsed option, and `agent.start`'s params, print variable names only in `Debug`. Output on
success names variables only, as before.

A4 A dangerous name is refused when it is set (amends D4). `agent.settings.set` refuses a variable whose name loads code,
redirects lookups, or swaps an endpoint or a trust store, using the same `is_dangerous_env_name` check the vault applies
to a secret's name (decision 0071), with `request.invalid_params` naming the variable only. Since every run that receives
standing variables now holds secrets (A1), such a name could never reach a run anyway: refusing it at set time reports
the problem where it is made instead of at every start. A consequence is that a standing variable cannot point a kind at
another endpoint through a `*_BASE_URL` or proxy variable; the standing program (a wrapper) remains the way to do that,
and loosening this needs its own decision.
