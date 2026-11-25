# 0160: Inherited environment and resume state

Status: accepted, 2026-11-24.

## Context

- When the desktop app is opened from inside another Claude Code session, the service inherits that session's
  environment and passes it to every terminal. A Claude session started by MassOrca then saw
  `CLAUDE_CODE_CHILD_SESSION`, took itself for a child session, and turned transcript saving off.
- The previous product (pv96) strips `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID` and
  `CLAUDE_CODE_BRIDGE_SESSION_ID` from every Claude child, comparing names without case on Windows.
- The same applies to MassOrca's own run identity: a service started from inside an agent run inherits
  `MASSORCA_AGENT_SESSION`, `MASSORCA_AGENT_TOKEN` and `MASSORCA_AGENT_KIND`. A run's launch sets its own, but a
  caller's `env` could override them, and a plain terminal kept the inherited ones.
- With transcript saving off, `agent resume` ran `claude --resume <ref>` with the right ref (learned from the hook or
  assigned with `--session-id`). The CLI answered "No conversation found with session ID" and exited 1, and nothing
  told the person beforehand that the session could not resume.

## Decision

- A kind may declare `strip_inherited_env`: variable names, each once, compared without case on Windows and exactly
  elsewhere. They are stripped from every run of the kind and from its account commands (check, sign-in, sign-out),
  through the existing paths: the launch plan's `remove_env` and `account_strip_names`. A name the launch itself sets
  is set, not stripped.
  - The built-in `claude` declares `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID`,
    `CLAUDE_CODE_BRIDGE_SESSION_ID` and `CLAUDECODE`.
  - `codex`, `antigravity` and `gentle-shell` declare none: pv96 strips no inherited session marker for them.
- The run identity is the launch's own. The caller's `env` no longer overrides `MASSORCA_AGENT_SESSION`,
  `MASSORCA_AGENT_TOKEN` or `MASSORCA_AGENT_KIND`. The service builds the base environment of every terminal without
  them, and the account commands strip them.
- Every session view carries `resume: { available, code?, reason? }`:
  - `agent.resume_unavailable` when the kind is not declared, invalid or not resumable, or no ref is known.
  - `agent.resume_not_found` with the reason "the agent saved no conversation for this session" when the kind
    declares `resume_transcript: { env?, dir, glob }` and, for an exited session, no file matches. The folder is the
    account home the run used, else the `env` variable's absolute local value, else `dir` with `{home}`. Only metadata
    is read and links count as missing. `claude` declares
    `{ "env": "CLAUDE_CONFIG_DIR", "dir": "{home}/.claude", "glob": "projects/*/{ref}.jsonl" }`.
  - The same code when a resumed run exits with a failure and the end of its output matches the kind's
    `resume_not_found_pattern` (`claude`: "No conversation found with session ID"). The session stays exited and its
    `exit` is recorded unchanged.
- `agent.resume` refuses with `agent.resume_not_found` instead of starting a run when the state says so.

## Consequences

- A Claude session started from a service that inherited another session's markers saves its transcript and can be
  resumed.
- The terminal can only add variables, so a stripped name is set to the empty string, not removed (the limitation of
  docs/architecture/agents.md). That an empty marker reads as unset to the vendor CLI is expected, not verified here;
  a program that checks only for presence still sees it.
- A caller that relied on setting the run identity through `env` no longer can.
- Every view of an exited session whose kind declares a transcript check lists the folders matched by the glob's `*`
  parts. Unit tests configure no vendor environment, so they never look outside the profile.
- Declarations gain three optional fields; a peer that predates them refuses a declaration that uses them.
