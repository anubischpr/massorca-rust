# 0096: Model and effort choices in kind declarations

Status: accepted, 2026-11-24.

## Context

Decision 0094 added `model` and `effort` to `agent.start`, and a kind declares the flag each one is passed with.
The desktop model picker needs more than the flag: the models a kind offers, and the reasoning levels each model
accepts. Only the built-in `claude` kind declared its flags, and the other built-in kinds declared none.

## Decision

- A kind's `model` and `effort` declare the choices the picker shows, beside their flag. Existing `{"flag": ...}`
  declarations stay valid.
  - `model.choices`: each `{id, label, default?, efforts?}`. `default: true` marks the model the picker preselects,
    and at most one choice has it. `efforts` lists the effort levels the model accepts. Absent means every level the
    kind declares; `[]` means the model takes no effort.
  - `effort.levels`: each `{id, label}`, in order. `effort.default` names one of them.
  - A value may reach the launch line as `args` instead of `flag`: a list in which `{value}` appears exactly once,
    such as `["-c", "model_reasoning_effort={value}"]`. Exactly one of `flag` and `args` is required.
- A start's effort must be one of the declared levels when the kind declares any. When the named model lists its
  efforts, the effort must be one of those. A refusal is `request.invalid_params` and names the allowed ids. A model
  id the declaration does not list still starts, since only the value format is checked (decision 0094).
- `agent.kinds` lists the choices: `model` and `effort` are present when the kind declares the option, and carry the
  choices and the levels. `agent kinds` prints a `models:` and an `efforts:` line for each kind that declares them,
  with the default marked.
- The built-in kinds declare the choices of the reference application (pv96) at commit d3c6311a531, read from its
  Claude, Codex and Antigravity option catalogues:
  - `claude`: models `fable`, `opus`, `sonnet` (default) and `haiku`, which takes no effort. Effort levels `low`,
    `medium`, `high` (default), `xhigh` and `max`.
  - `codex`: models `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5` and `gpt-5.2-codex`, with no default.
    Each model lists the levels up to its ceiling: `ultra` for sol and terra, `max` for luna, and `xhigh` for
    gpt-5.5 and gpt-5.2-codex. Levels run from `minimal` to `ultra`, and the default is `medium`. The model is passed
    as `-m <model>` and the effort as `-c model_reasoning_effort=<level>`.
  - `antigravity`: no static models (`choices` is empty), and effort levels `low`, `medium` and `high`, with `high`
    as the default.
  - `gentle-shell` declares neither. Its model and thinking level are set live over its RPC, and it has no launch flag
    for them.
- Choices are a static seed. The reference also discovers each account's models from the vendor CLI. Discovery is not
  ported yet, so a kind with an empty `choices` list is account-scoped: the picker shows no models for it until
  discovery lands.
- Left out, on purpose:
  - Speed (Fast). The reference passes no launch flag for it on any CLI kind. Claude toggles `/fast` in a running
    session, and Codex offers it only on its structured path.
  - Gentle Shell's model and thinking level. They are set live over RPC, with no launch flag.

## Consequences

- The three built-in declarations gain `model` and `effort` choices. Each packaged copy under
  `packages/agents/massorca.<kind>/agents/` stays byte-identical to its built-in declaration in
  `crates/core-agents/kinds/`, which the catalogue test checks.
- `agent.kinds` gains the optional `model` and `effort` objects. A peer that predates them decodes the kind without
  them, and no capability is added.
- A vendor that retires a model keeps it listed until the declaration changes. The start still passes an unlisted
  model id through, as decision 0094 allows.
