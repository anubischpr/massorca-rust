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
