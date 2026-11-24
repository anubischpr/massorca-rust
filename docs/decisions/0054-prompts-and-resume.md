# 0054: Prompt delivery and resume

Status: accepted, 2026-11-21.

## Context

The old app had six ways to give a terminal agent its first prompt, and for
later prompts pasted text into the agent's composer after waiting for a
"ready" signal, often a quiet-screen timer (`docs/inventory/agents.md`,
AG-05). A terminal keeps no transcript, so a delivery could only ever be
reported as `handed-to-terminal`. Resume rebuilt a vendor command from the
agent's own conversation id (AG-11).

## Decision

- **First prompt at launch**, by the kind's `prompt` mechanism: `argv` (the
  text is the last argument, after `--` when the declaration asks for it),
  `flag` (the declared flag, then the text) or `none` (the kind cannot take
  one; a prompt given to `agent.start` is then delivered as a later prompt
  once the agent reports `idle`). Text over 8 KiB or with a NUL is refused
  at launch (`agent.prompt_invalid`); long briefs go through a file the
  prompt names.
- **Later prompts: `agent.prompt`** types the text into the session's
  terminal: the text, then the kind's submit sequence (default `\r`), as
  one terminal input. Kinds that declare `paste: bracketed` get the text
  wrapped in bracketed-paste markers so multi-line text is not submitted
  line by line. Readiness is the agent's reported activity, never a screen
  timer: `agent.prompt` is accepted when activity is `idle` or `done`, or
  when the kind has no status mechanism. Otherwise it is refused with
  `agent.busy` (activity `working` or `waiting`), unless `force` is set. An
  exited run refuses with `agent.not_live`.
- **The receipt is honest**: `agent.prompt` returns `handed_to_terminal`
  with the byte count. It never claims the agent read or started the turn;
  the activity moves to `working` only when the agent's hook says so.
- **Resume: `agent.resume`** starts a new run of an exited session with the
  kind's resume arguments, `{ref}` replaced by the session's `agent_ref`.
  - Claude: MassOrca generates the ref at the first launch and passes
    `--session-id <ref>`, so every Claude session is resumable from the
    start.
  - Codex: the ref is learned from the turn-complete payload.
  - A kind without a resume form, or a session whose ref is unknown, is
    refused with `agent.resume_unavailable`.
  - Resume runs in the same workspace and keeps the one-writer rule (0051).
  The new run gets a new terminal and token; activity starts absent until
  the agent reports.
- **Sessions per workspace**: `agent.list --workspace` lists the sessions
  MassOrca started there, newest first, live and exited. Importing sessions
  started outside MassOrca (HIST-xx) is not in Phase 4.

## Alternatives

- **Wait for a quiet screen** before typing. Timing-dependent and wrong on
  a loaded machine; the activity report says exactly when the composer
  waits.
- **Queue prompts while the agent works.** Useful, but it needs a delivery
  journal to survive restarts; a later structured-chat adapter brings one.

## Consequences

- Prompt delivery to agents without hooks (Antigravity, Gentle Shell for
  now) relies on the person knowing the agent is ready.
- A prompt typed while an agent shows a permission question would answer
  the question; `force` exists for exactly that, deliberately.

## Amendment: startup-query mode (2026-11-24)

The first prompt can also reach the agent as its startup query, `prompt.mode: "startup-query"`
(inventory AG-05). The source is the Hermes startup query of pv96, commit `d3c6311a531`, read only:

- `src/shared/tui-agent-config.ts:285`: Hermes declares `promptInjectionMode: 'hermes-query'`.
- `src/shared/tui-agent-startup.ts:128-147`: the plan has `followupPrompt: null`, and the comment at 143
  says Hermes owns readiness and submission for `chat --query`. Orca never types the prompt.
- `src/shared/hermes-startup-query.ts:132-138`: the invocation is the kept arguments with `chat`, then
  `--query=<text>`, then `--tui`. The `chat` subcommand must come before the query flag.
- `src/shared/hermes-startup-query.ts:17`, `:161`, `:190`: pv96 carries the text in the environment variable
  `ORCA_HERMES_STARTUP_QUERY` and a `sh -c` wrapper that eval-expands it into that argument, because it types
  one shell line. The running Hermes receives the text as an argument either way.

Decision: a `startup-query` mode, named for the generic contract rather than pv96's agent-specific
`hermes-query`. The launch plan adds one argument, `<flag>=<text>`, after the kind's and the caller's
arguments, as `flag` mode does, but as one token, so text that starts with `-` cannot read as an option.
The `flag` is required and holds no `=`, and a `separator` is refused. The run holds no prompt and MassOrca
types nothing later, so no readiness signal is needed; the agent submits the query itself. The environment
variable and the shell wrapper are not adopted: the spawn takes argv directly, with no shell line to quote.
A kind that uses the mode declares its subcommand in `args` (for Hermes, `chat`, then `--tui`).

Not verified against a real Hermes binary. The fake agent accepts `--query=<text>` as its prompt, which
covers the argument shape and the launch path only. No built-in kind uses the mode yet.
