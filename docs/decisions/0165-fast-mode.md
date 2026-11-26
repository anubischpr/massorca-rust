# 0165: Fast mode on agent launches

Status: accepted, 2026-11-25.

## Context

Decision 0096 left Fast out of the kind choices on purpose: the reference passes no launch flag for it on any CLI
kind. Claude toggles `/fast` in a running session, and Codex offers it only on its structured path. The model and
effort work (0094, 0096) and the inventory's model, effort and mode selection need a start that can ask for fast
mode, so the desktop and the CLI can offer it where the kind supports it.

What is known about Claude Code, from its documentation and the reference application's catalogue:

- There is no `--fast` flag.
- Fast mode is the interactive `/fast` toggle, the `fastMode` setting, or a `--settings` object with `fastMode`.
  The documentation shows the settings form with `-p`. Whether the settings form takes effect in an interactive run
  is not verified.
- Only some models take it. The reference marks it per model (`supportsFastMode`): its static Claude catalogue
  offers it on `opus` only, and its host probe may report it per account.

## Decision

- A kind declares fast mode as `fast`: `{ "settings": {...}, "settings_flag"?: "--settings", "live"?: "/fast",
  "toggle"?: bool }`.
  - `settings` is a non-empty object and may not set `hooks`, which the status mechanism owns.
  - `settings_flag` is a flag, `--settings` by default. A start with fast mode passes `settings` on it as one JSON
    argument, with no shell in between.
  - A `claude-settings` run already passes its hook settings file on `--settings`. When the settings flag is
    `--settings` too, the fast keys merge into that file instead of a second `--settings`.
  - `live` and `toggle` are declared for the live switch of a later change (`agent.set_options`). Nothing sends them
    now.
- A `model.choices` entry may set `fast: true`. Only a kind that declares `fast` may set it.
- The built-in `claude` kind declares `fast` with `{"fastMode": true}`, `live` `/fast` and `toggle`. Only `opus` takes
  it, as in the reference's static catalogue. `fable`, `sonnet` and `haiku` do not.
- A start with fast mode on is refused with `agent.option_unsupported`, naming the kind and the model, when:
  - the kind declares no `fast`;
  - the kind lists model choices and the named model, or the default choice when no model is named, is not marked
    `fast: true`.
  A kind with no model choices has nothing to check, so it passes fast mode on for any model. Without `--model`,
  Claude's default is `sonnet`, so `--fast` on Claude needs `--model opus`.
- Wire (additive, serde defaults; capability `agent-fast/1`):
  - `agent.start` takes `fast?: bool`. Absent is the CLI's default; `false` passes nothing.
  - `agent.kinds` lists `fast?: { supported, reason? }` on each kind: `supported` when it declares `fast`, otherwise
    `false` with the reason. `model.choices[].fast` is omitted when false.
  - An `AgentSession` carries `fast?: { on, confirmed }` when its current or last run was started with fast mode on.
    `confirmed` is always false for now, since nothing confirms the vendor took it.
  - The client refuses `fast` before sending when the service does not offer `agent-fast/1`.
- A start with fast mode records the request in the run directory, and the session view reads it from there. A resume
  takes no fast mode, so it clears the record, and the session shows no fast mode until a new start asks for it.
- The CLI takes `massorca agent start --fast`. `agent status` shows `fast: requested` while the request is not
  confirmed.
- No mid-session toggle is implemented here. It comes with `agent.set_options`.

## Needs a real-CLI check

Before a live switch or a claim that the vendor took fast mode:

- Whether `--settings` with `{"fastMode": true}` takes effect in an interactive Claude Code run, or only with `-p`.
- What `/fast` does in a running session, and whether it is a toggle or a set.
- Whether the per-model support in the reference's probe differs from the static catalogue.

Until those are checked, `confirmed` stays false, and a session reports `fast: requested` only.

## Consequences

- Fast mode is optional and off by default. A start that does not ask for it passes no settings and writes no
  fast keys.
- The `claude` kind's settings file, when hooks use it, gains a `fastMode` key only for a start that asked for fast
  mode.
- Other built-in kinds declare no fast mode and refuse it.
- Delivery of `--fast` to the desktop is a later change. This decision adds only the optional fields the desktop
  needs to decode, and the capability check before sending.
