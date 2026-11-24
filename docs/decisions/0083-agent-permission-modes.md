# 0083: Agent permission modes

Status: accepted, 2026-11-23.

## Context

A person starting an agent from MassOrca wants to choose how much it may do without asking: plan only, accept edits,
run everything. Each vendor CLI has its own modes, flags and in-session switches. The desktop needs to start an agent
in a mode, show the mode a session is in, and change it. Before this decision a kind declaration (0050) said nothing
about modes, so the only way to choose one was to pass the vendor's flag after `--` by hand.

Rulings from the owner (2026-11-23): bypass modes ("yolo mode") are offered like any other mode; they are never the
default, are marked `bypass: true`, and are chosen explicitly per start or switch; they are not gated by device scope.
A Bot Teams task start chooses through its team's access instead: `full` names the kind's bypass mode (decision 0072,
amendment "team access").
A switch that no hook confirms is reported as requested, not applied. On the default (2026-11-23, superseding the first
version of D3 and D4 for it): "When the person has chosen no mode, MassOrca passes NO mode flag and the vendor CLI starts in
its own default, exactly as if they ran it themselves; the declared `default_mode` is the id of that vendor default so the
chip can name it [...] and the session records "vendor default" rather than pretending a mode was set. A flag is passed
only when the person picked a mode."

## Vendor evidence

Every mode below was read from the vendor's own documentation on 2026-11-23. Flags were also checked against each
installed CLI's `--help`, run with an empty temporary home (Claude Code 2.1.268, Codex 0.159.2, Antigravity CLI 1.5.11;
Gentle Shell is not installed here). A mode the documentation does not define, or whose launch form it does not give,
is not declared.

Defaults (the vendor CLI's own, named for display only): `claude` `auto` (`default` before 2.1.283), `codex` `auto`,
`antigravity` `auto`, `gentle-shell` `normal`. Modes marked `may_fall_back`: `claude` `auto`, `antigravity` `auto`.

| Kind | Mode id | Label (the vendor's name) | How it is passed when chosen | Bypass |
|---|---|---|---|---|
| `claude` | `default` | Manual | `--permission-mode default` | |
| `claude` | `acceptEdits` | Edit automatically | `--permission-mode acceptEdits` | |
| `claude` | `plan` | Plan | `--permission-mode plan` | |
| `claude` | `auto` | Auto | `--permission-mode auto` | |
| `claude` | `dontAsk` | Don't ask | `--permission-mode dontAsk` | |
| `claude` | `bypassPermissions` | Bypass permissions | `--permission-mode bypassPermissions` | yes |
| `codex` | `auto` | Auto | `--sandbox workspace-write --ask-for-approval on-request` | |
| `codex` | `read-only` | Safe read-only browsing | `--sandbox read-only --ask-for-approval on-request` | |
| `codex` | `read-only-never` | Read-only non-interactive (CI) | `--sandbox read-only --ask-for-approval never` | |
| `codex` | `auto-review` | Auto-review mode | `--approve-for-me` | |
| `codex` | `full-access` | Dangerous full access | `--dangerously-bypass-approvals-and-sandbox` | yes |
| `antigravity` | `auto` | auto | `--mode=auto` | |
| `antigravity` | `accept-edits` | accept-edits | `--mode=accept-edits` | |
| `antigravity` | `plan` | plan | `--mode=plan` | |
| `antigravity` | `always-proceed` | always-proceed | `--dangerously-skip-permissions` | yes |
| `gentle-shell` | `normal` | Normal | live: `/gentle:mode normal` | |
| `gentle-shell` | `auto` | Auto | live: `/gentle:mode auto` | |
| `gentle-shell` | `yolo` | YOLO | live: `/gentle:mode yolo` | |
| `gentle-shell` | `super-yolo` | Super Yolo | live: `/gentle:mode super-yolo` | yes |

Sources and what each settled:

- Claude Code, https://code.claude.com/docs/en/permission-modes (read 2026-11-23). The six modes are the values of
  `--permission-mode`; labels are the names the page gives the mode indicator ("Manual", "Edit automatically", "Plan",
  "Auto", "Bypass permissions"). The indicator table has no entry for `dontAsk`; the status bar shows "don't ask on",
  so the label stays "Don't ask". `--dangerously-skip-permissions` is the same as
  `bypassPermissions`, which the page limits to isolated containers and VMs. Switching in a session is `Shift+Tab`,
  whose cycle depends on settings and version, so it is not declared as a live switch. The vendor's own starting mode
  is `auto` on 2.1.283 and later and `default` before; the declared default is `auto`. `auto` may fall back: "When the
  flag, a settings file, or the built-in default selects `auto` but auto mode isn't available to the session, Claude
  Code starts the session in Manual instead."
- Codex, https://learn.chatgpt.com/docs/agent-approvals-security and
  https://learn.chatgpt.com/docs/developer-commands?surface=cli (both read 2026-11-23). The first page's table of
  "common sandbox and approval combinations" names "Auto (preset)", "Safe read-only browsing", "Read-only
  non-interactive (CI)", "Auto-review mode" and "Dangerous full access"; those are the labels, each with the flags the
  table gives. The page gives "Auto-review mode" as `-c approvals_reviewer=auto_review`; the developer-commands page
  documents `--approve-for-me` for the same review with `on-request` and `workspace-write`, and that flag is declared.
  Only "Auto" is called a preset, so the other ids come from their flag values. `--full-auto` is on neither page nor in
  the installed help and is not used. The vendor starts in "Auto" in a version-controlled folder and may start in
  `read-only` in another; the declared default is `auto`. `/permissions` is an interactive picker and is not a live
  switch.
- Antigravity CLI, https://www.antigravity.google/docs/cli/modes (read 2026-11-23; the settings page
  https://antigravity.google/docs/settings?app=cli was read by the research of the same day). The `default` mode has no
  launch flag: it is set in `settings.json`, and `--mode=default` is not accepted by 1.5.11. Since 1.5.7, `agy` starts in
  `auto` where the account and platform allow it and in `default` otherwise, and `--mode=auto` on an unsupported setup
  runs in `default`. So `default` cannot be chosen at launch and is not declared; the declared default is `auto`, the
  vendor's own start. `auto` may fall back: "Setting `agentMode` to `"auto"` or passing `--mode=auto` on an unsupported
  setup runs in `default`." `--dangerously-skip-permissions` is a global override the page calls
  `always-proceed`. `Shift+Tab` cycles and `/config` is interactive, so no live switch is declared.
- Gentle Shell, https://github.com/Gentleman-Programming/gentle-shell/blob/main/docs/permission-modes.md (read
  2026-11-23). Four modes, Normal the default. No launch flag is documented. `/gentle:mode <id>` switches the current
  session and is a deterministic command, so it is each mode's live switch. Super Yolo removes every prompt and is
  marked bypass. YOLO keeps destructive confirmations and is not marked bypass.

## Decision

**D1, schema.** A kind declares
`permission_modes: [{ id, label, description?, args, live_switch?: { type: "input", text }, bypass?, may_fall_back? }]`
and `default_permission_mode`. Ids are the vendor's spelling, `[A-Za-z0-9-]{1,32}`, and unique. `args` may be empty, but
a mode with neither `args` nor `live_switch` can only be the default. The default is one of the ids and never a bypass
mode. A bypass mode has its own `args` or `live_switch`. `may_fall_back` marks a mode the vendor documents starting
another mode in without failing. A kind whose documentation defines no mode declares none. `agent.kinds` lists each
mode's `id`, `label`, `description`, `bypass`, `live_switch` (whether one is declared) and `may_fall_back`; the arguments
and switch text stay in the declaration.

**D2, live switches.** Only a deterministic command the vendor documents is a live switch (today, Gentle Shell's
`/gentle:mode <id>`). Cycling keys and interactive pickers are not: their result depends on state MassOrca cannot see.
Those kinds change mode at the next resume.

**D3, the default is the CLI's own.** When no mode is chosen, no mode flag is passed and nothing is typed. The session
records `mode_source: vendor_default` and no `permission_mode`. `default_permission_mode` names the vendor's own default
for display only; the label comes from the declaration and may lag a vendor that changes its default with a version.
It is informational and must never be shown as a session's mode: a `vendor_default` session is labelled "the CLI's
default". A chosen mode that has a live switch but no `args` is applied by typing its switch at the run's first `idle`,
before a held first prompt (0054). A kind that reports no `idle` cannot do this, so such a mode is refused at start with
`agent.mode_unavailable` (Gentle Shell today); `agent.set_mode` switches it once the run is up.

**D4, `mode_confirmed`.** A session reports `permission_mode`, `mode_source` (`vendor_default` or `chosen`) and
`mode_confirmed`. `mode_confirmed` is true only when the chosen mode was passed as launch arguments of the current run,
no caller arguments followed (they may override it), and the mode is not `may_fall_back`. It is false for a typed live
switch, which no hook confirms, and for the vendor's default, about which nothing is claimed. A hook payload that reports
the mode may set it true later; no vendor documents one yet.

**D5, Codex.** Codex's modes are the named combinations of its documentation, each mapped to documented flags, as listed
above.

**D6, switching and resuming.** `agent.set_mode` types a live switch into a live run and records the mode as chosen and
unconfirmed (`applied: live`). Otherwise it stores `pending_permission_mode` and leaves the running mode and its
confirmation alone (`applied: next_resume`); a mode a resume could not apply (only a live switch, on a kind that reports
no `idle`) is refused with `agent.mode_unavailable`. A live switch into a kind with no status mechanism needs `force`,
because MassOrca cannot tell whether its agent is in the middle of a prompt. While a switch is typed the run is marked as
switching, and every other prompt or switch is refused with `agent.busy`. A resume uses, in order: the mode it names,
which also ends any wait; the pending mode, unless it is a bypass mode, which is used only by a resume that names it and
otherwise stays pending while the run gets the vendor's default; the last chosen mode, unless it is a bypass mode; the
vendor's default. A bypass mode is never carried into a run without being named for it. A delegated child gets the mode
its delegation names or the vendor's default, never its parent's; a delegation carrying a run token (an agent asking) may
not name a bypass mode, which the person chooses through `agent.start`.

**D7, errors.** A mode id the kind does not declare, or any mode for a kind that declares none, is refused with
`agent.mode_unknown`; its data names the kind, the mode and the known ids. `agent.mode_unavailable` refuses a mode that
cannot be applied (D3, D6), a live switch without `force` into a kind with no status mechanism, and a bypass mode in an
agent's own delegation.

The wire is additive: `agent.kinds` gains the two kind fields and each mode's `may_fall_back` (PR A); `agent.start`,
`agent.delegate` and `agent.resume` gain `permission_mode`, the session gains `permission_mode`, `mode_source`,
`mode_confirmed` and `pending_permission_mode`, and `agent.set_mode` (`session`, `permission_mode`, `force?`) comes with
capability `agent-modes/1` (PR B). Bypass modes are not gated by device scope: the existing `workspace` permission of each
method applies.

**Future work.** A saved per-kind mode preference, which would count as a chosen mode, is not part of this decision.

## Consequences

- A start with no mode runs exactly as before this decision: no mode flag, the vendor's own default.
- A vendor that changes its default with a version is followed, not overridden; only the label shown for the default
  may lag until the declaration changes.
- The package copies of the four kind files under `packages/agents/` change with the core copies, byte for byte (0062);
  their content hash changes with them.
- A vendor that adds or renames a mode needs a declaration change; an undeclared mode can still be passed after `--`.
