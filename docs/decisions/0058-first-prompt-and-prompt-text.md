# 0058: First prompts and prompt text

Status: accepted, 2026-11-21.

## Context

Decision 0054 left open what a first prompt does when its agent never reports `idle`, what text a
prompt may carry, and whether a second prompt may follow a first before the agent has reported
anything. The first implementation settled these in code, and an independent review then found
three faults: the end marker was stripped in a single pass, so a spliced marker came back alive, and
escape sequences and C1 controls reached the agent in both paste modes; a second prompt was accepted
on the same `idle`; and a stale exit recorded on a resumed session turned the new run into an exited
one. This record fixes the rules the code now follows.

## Decision

- **A held first prompt waits at most 120 s.** A kind whose `prompt.mode` is `none` receives its
  first prompt at its first `idle` report; the prompt is held on the run from `agent.start`. The
  hold is bounded: the prompt is dropped when the run exits first, or when no `idle` arrives within
  120 s of the start (`FIRST_PROMPT_WAIT`). A drop is logged with the session id only. The session
  record does not show a drop; recording one needs a wire field or a persisted column, and neither
  is added here.
- **A kind that cannot report `idle` refuses such a prompt at launch.** A kind with no status
  mechanism, or whose events map no event to `idle`, gets `agent.prompt_invalid` from `agent.start`,
  before anything runs. A prompt that could never be sent is not accepted.
- **Prompt text holds no control character except tab, line feed and carriage return.** NUL, the
  escape character (0x1B), DEL, the other C0 controls and the C1 controls (U+0080 to U+009F) are
  refused with `agent.prompt_invalid`, at launch and in `agent.prompt`, in both paste modes. Because
  the escape character is refused, bracketed text cannot hold a paste end marker, so it is wrapped
  without further escaping.
- **A prompt is followed by a report before the next prompt.** A run that has taken a prompt, by
  `agent.prompt` or as a held first prompt, carries a typed-since-report flag. The flag is set under
  the status lock when the prompt is accepted, and the next accepted hook report clears it. While it
  is set, `agent.prompt` is refused with `agent.busy` unless `force` is given. A kind with no status
  mechanism never sets the flag, since nothing reports back to clear it, so it stays always ready as
  0054 says.
- **An exit is recorded against the terminal it ran in.** A run's end is written only while the row
  still names that terminal. An end that arrives after a resume has moved the session onto a new
  terminal changes nothing, and does not end the resumed run.
- **Resume keeps the title and the last result, not the first start's arguments and environment.**
  The resumed run gets the kind's own arguments and its resume arguments. The caller's `args` and
  `env` of the first start are not stored with the session. A session whose last run has no recorded
  end (no terminal and no exit) cannot resume and gets `agent.resume_unavailable`.

## Alternatives

- **Strip the end marker from the text.** A single replace pass can be spliced back together, and
  the escape character is what the rule must keep out of the text anyway; the text is refused
  instead.
- **Wait for `idle` without a bound.** An agent that never reports `idle` would hold the text for
  as long as its run lives.
- **Record a dropped first prompt on the session.** Needs a wire field or a persisted column; deferred.
- **Apply the typed-since-report flag to every kind.** A kind with no status mechanism could never
  take a second prompt without `--force`, which contradicts 0054.
- **Store the first start's arguments and environment for resume.** The environment a caller passes
  can hold secrets, and the session record holds none today.

## Consequences

- A dropped held prompt is visible only in the service log; the session keeps showing `no status`
  until the agent reports.
- After a prompt to an agent that never reports, the next prompt needs `--force`. That is the
  deliberate escape, as 0054 says for a question the agent is asking.
- Kinds with no status mechanism keep accepting prompts back to back, since nothing can tell
  MassOrca that the first one was read.
- A resumed run starts with the kind's arguments only. The resume command has no way to pass more
  yet.
