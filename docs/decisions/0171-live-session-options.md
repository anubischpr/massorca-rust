# 0171: Live session options

Status: accepted, 2026-11-26.

## Context

Decision 0094 left a resumed run without the model or effort of its first start; decision 0168 (#412) keeps them, stored
with the session, where the kind still takes them. A resume takes no fast mode (decision 0165). The desktop wants to change a session's model, effort and fast mode while it runs, or else at its next
resume. Claude Code takes `/fast` as a toggle in a running session (decision 0165). The vendor's slash-command reference
does not list `/model` or `/effort`, and the public docs do not document a `set_model` control request for headless
stream-json, so no control request is sent and `/model` is declared as unverified.

## Decision

- Method `agent.set_options { session, model?, effort?, fast? }`, capability `agent-options-live/1`. An option left out
  keeps its value; at least one is required. It has workspace permission, as `agent.set_mode` does: a live change into a
  run that holds vault secrets needs host-admin scope.
- A model with a live command (`model.live` in the kind declaration), and the fast toggle (`fast.live`), are typed into
  a running terminal run when the agent is ready for input. A busy run refuses the change with `agent.busy`, as a mode
  switch does, and types nothing. Effort has no live command, so it always waits, and so does every
  change to a run that is not live. A chat run has no terminal, so its changes always wait.
- Fast mode is a toggle, so its command is typed only when the run's recorded state differs from the state asked for.
  `session.fast` stays `confirmed: false`, since nothing confirms the vendor took it.
- A typed live command starts a settle window of 2 seconds (`TYPED_COMMAND_SETTLE`). Until it ends, non-forced prompts,
  live changes and the queue wait (`agent.busy`); `force` and `send_now` pass. An accepted hook report ends the window
  early. Otherwise a timer ends it and drains the queue, so a queued prompt is typed once the window ends, with no report.
  Reason: a slash command may send no hook report (no `UserPromptSubmit`, no `Stop`), so a hold that only a report
  clears could keep the session busy forever. A mode switch starts no window, as before.
- Values are checked as `agent.start` checks them, with the same refusals. A refused value changes nothing.
- The model and effort a session keeps for its next run are its stored columns (decision 0168). A `next_resume` change
  writes them, an applied live change writes them too, and a resume launches with them where the kind still takes them.
  Fast mode has no column, so its chosen value is held in memory and dropped on forget. `started`, what the running
  process uses, stays in memory, and a live change is checked against it.
- The result is `{ applied: { model?, effort?, fast? }, session }`. Each changed option is `live` or `next_resume`; an
  option already at its value is absent from `applied`, and nothing is typed for it. Its value is still kept for the
  next resume.

## Consequences

- No existing type changes shape. The params and the result are new types, and the kind declaration gains an optional
  `model.live`.
- Unverified: `/model {value}` for the built-in `claude` kind, and `/fast`, which the public slash-command page does not
  list. `/model` or `/fast` might open a confirmation screen; the settle window then holds prompts and live changes
  (`agent.busy`) for 2 seconds, or until a report. The timing of typed commands sent one after another is not verified
  either: each command is written in turn, while the run is marked as switching.
- A service restart drops a stopped session's chosen fast mode. Its model and effort are stored and survive.
- Decision 0168 says the stored columns name what a session's latest run used. A `next_resume` change writes them before
  the next run starts, so until that run starts they name the options it will take.
