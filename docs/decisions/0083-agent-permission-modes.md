# 0083: Agent permission modes

Status: accepted, 2026-11-23.

## Context

A person starting an agent from MassOrca wants to choose how much it may do without asking: plan only, accept edits,
run everything. Each vendor CLI has its own modes, flags and in-session switches. The desktop needs to start an agent
in a mode, show the mode a session is in, and change it. Before this decision a kind declaration (0050) said nothing
about modes, so the only way to choose one was to pass the vendor's flag after `--` by hand.

Rulings from the owner (2026-11-23): bypass modes ("yolo mode") are offered like any other mode; they are never the
default, are marked `bypass: true`, and are chosen explicitly per start or switch; they are not gated by device scope.
A switch that no hook confirms is reported as requested, not applied.

## Vendor evidence

Every mode below was read from the vendor's own documentation on 2026-11-23. Flags were also checked against each
installed CLI's `--help`, run with an empty temporary home (Claude Code 2.1.268, Codex 0.159.2, Antigravity CLI 1.5.11;
Gentle Shell is not installed here). A mode the documentation does not define, or whose launch form it does not give,
is not declared.

| Kind | Mode id | Label (the vendor's name) | How it is passed | Bypass |
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
  "Auto", "Bypass permissions") and the status line ("don't ask"). `--dangerously-skip-permissions` is the same as
  `bypassPermissions`, which the page limits to isolated containers and VMs. Switching in a session is `Shift+Tab`,
  whose cycle depends on settings and version, so it is not declared as a live switch. The vendor's own starting mode
  depends on the version and plan (`auto` on 2.1.283 and later, `default` before), so MassOrca names it: the declared
  default is `default`, passed as a flag.
- Codex, https://learn.chatgpt.com/docs/agent-approvals-security and
  https://learn.chatgpt.com/docs/developer-commands?surface=cli (both read 2026-11-23). The first page's table of
  "common sandbox and approval combinations" names "Auto (preset)", "Safe read-only browsing", "Read-only
  non-interactive (CI)", "Auto-review mode" and "Dangerous full access"; those are the labels, each with the flags the
  table gives. The page gives "Auto-review mode" as `-c approvals_reviewer=auto_review`; the developer-commands page
  documents `--approve-for-me` for the same review with `on-request` and `workspace-write`, and that flag is declared.
  Only "Auto" is called a preset, so the other ids come from their flag values. `--full-auto` is on neither page nor in
  the installed help and is not used. The vendor starts in "Auto" in a version-controlled folder and may start in
  `read-only` in another; MassOrca passes "Auto" explicitly. `/permissions` is an interactive picker and is not a live
  switch.
- Antigravity CLI, https://www.antigravity.google/docs/cli/modes (read 2026-11-23; the settings page
  https://antigravity.google/docs/settings?app=cli was read by the research of the same day). The `default` mode has no
  launch flag: it is set in `settings.json`, and `--mode=default` is not accepted by 1.5.11. Since 1.5.7, `agy` starts in
  `auto` where the account and platform allow it and in `default` otherwise, and `--mode=auto` on an unsupported setup
  runs in `default`. So `default` cannot be chosen at launch and is not declared; the declared default is `auto`, which
  behaves as the vendor's own start. `--dangerously-skip-permissions` is a global override the page calls
  `always-proceed`. `Shift+Tab` cycles and `/config` is interactive, so no live switch is declared.
- Gentle Shell, https://github.com/Gentleman-Programming/gentle-shell/blob/main/docs/permission-modes.md (read
  2026-11-23). Four modes, Normal the default. No launch flag is documented. `/gentle:mode <id>` switches the current
  session and is a deterministic command, so it is each mode's live switch. Super Yolo removes every prompt and is
  marked bypass. YOLO keeps destructive confirmations and is not marked bypass.

## Decision

**D1, schema.** A kind declares
`permission_modes: [{ id, label, description?, args, live_switch?: { type: "input", text }, bypass? }]` and
`default_permission_mode`. Ids are the vendor's spelling, `[A-Za-z0-9-]{1,32}`, and unique. `args` may be empty, but a
mode with neither `args` nor `live_switch` can only be the default. The default is one of the ids and never a bypass
mode. A bypass mode has its own `args` or `live_switch`. A kind whose documentation defines no mode declares none.
`agent.kinds` lists each mode's `id`, `label`, `description`, `bypass` and `live_switch` (whether one is declared); the
arguments and switch text stay in the declaration.

**D2, live switches.** Only a deterministic command the vendor documents is a live switch (today, Gentle Shell's
`/gentle:mode <id>`). Cycling keys and interactive pickers are not: their result depends on state MassOrca cannot see.
Those kinds change mode at the next resume.

**D3, a mode with no launch flag.** A mode that has a live switch but no `args` is applied at start by typing its switch
at the run's first `idle`, as a first prompt is for a kind that takes none at launch (0054); the switch goes before that
prompt. A kind that reports no `idle` cannot do this: its default mode is recorded unconfirmed and nothing is typed,
and naming such a mode at start is refused with `agent.mode_unavailable`. Use `agent.set_mode` once the run is up.
Gentle Shell is such a kind today (its status mechanism is `none`).

**D4, `mode_confirmed`.** A session reports `permission_mode` and `mode_confirmed`. `mode_confirmed` is true when the
mode was passed as launch arguments of the current run, and false when it was only requested: a live switch typed with
no hook to confirm it, a mode stored for the next resume, or a default with no way to apply it. A hook payload that
reports the mode may set it true later; no vendor documents one yet.

**D5, Codex.** Codex's modes are the named combinations of its documentation, each mapped to documented flags, as listed
above.

**D6, bypass on resume.** A bypass mode is never carried over silently. A resume uses, in order: the mode the resume call
names; a mode `agent.set_mode` stored for the next resume (an explicit choice, bypass or not); the session's last mode
when it is not bypass; otherwise the kind's default. A delegated child never inherits its parent's mode: it gets the mode
its delegation names, or its kind's default.

**D7, errors.** A mode id the kind does not declare, or any mode for a kind that declares none, is refused with
`agent.mode_unknown`; its data names the kind, the mode and the known ids. A mode that cannot be applied at start (D3) is
refused with `agent.mode_unavailable`.

The wire is additive: `agent.kinds` gains the two kind fields (T1), `agent.start` and `agent.delegate` gain
`permission_mode`, the session gains `permission_mode` and `mode_confirmed`, and capability `agent-modes/1` is offered
(T2); `agent.set_mode` and the resume's `permission_mode` come with it (T3). Bypass modes are not gated by device scope:
the existing `workspace` permission of each method applies.

## Consequences

- A start with no mode now passes the kind's default flag (`--permission-mode default` for Claude, the "Auto" flags
  for Codex, `--mode=auto` for Antigravity). Before, nothing was passed and the vendor's own default applied; for Claude
  2.1.283 and later that was `auto`, so a start that names no mode now runs in Manual.
- The package copies of the four kind files under `packages/agents/` change with the core copies, byte for byte (0062);
  their content hash changes with them.
- A vendor that adds or renames a mode needs a declaration change; an undeclared mode can still be passed after `--`.
