# 0163: Permission modes as data, and archive and delete of agent sessions

Status: accepted, 2026-11-25.

## Context

The desktop's mode picker needs more than an id and a label: a one-line description, an icon, a tone that marks the modes
that lift the vendor's prompts, the order to show them in, and which modes to offer at all. Hard-coding that in the client
would drift from the kind declarations of decision 0083.

A finished session stays in every list until it is forgotten, and forgetting it leaves its worktree and branch behind.
The person wants to hide a session without losing anything, and to delete one together with its workspace, after seeing
what the delete would meet.

## Decision

**D1, mode presentation.** A declared permission mode may carry `icon` (a lucide icon name: lowercase letters and digits
in `-`-joined words, at most 64 characters), `tone` (`normal` or `warning`) and `selectable` (default true), next to its
`description`. The kind's `problems` name a tone outside the two values, an icon that is not such a name, and a bypass
mode whose tone is `normal`; a bypass mode that names no tone is listed as `warning`. Declared order is picker order. `agent.kinds` lists `icon`, `tone` and `selectable` as
optional fields: an older peer omits them, and an absent `selectable` means offered.

**D2, the built-in modes.** Every declared mode of the four built-in kinds has a description, an icon and a tone; every
bypass mode is `warning` and only those are. Claude offers, in order: `auto` "Auto", `default` "Manual", `acceptEdits`
"Accept edits", `plan` "Plan", `bypassPermissions` "Bypass permissions". `dontAsk` stays declared with
`selectable: false`: a session that reports it still shows its name, and a caller that names it is still served, but
pickers do not offer it. The owner chose the label "Accept edits" for `acceptEdits` over the vendor's indicator name
"Edit automatically" that decision 0083 recorded. Claude's default and `may_fall_back` are unchanged. The Codex,
Antigravity and Gentle Shell mode ids, labels and flags were checked against decision 0083's recorded findings and match
them; only their presentation fields were added. Separately, the installed `agy` 1.5.17 `--help` lists `--effort` values
`low|medium|high|xhigh|max`, so the Antigravity kind adds `xhigh` "Extra high" and `max` "Max" to its effort levels; its
default stays `high`. The package copies under `packages/agents/` change byte for byte with
the core copies.

**D3, archive.** `agent.archive_session { session, archived }` sets or clears a durable `archived_at` on the session
(one appended migration step). `AgentSession.archived` is sent when true. `agent.list` leaves archived sessions out
unless `include_archived: true`; `agent.show`, `agent.watch` and the service's own reads still see them. A live run is
refused with `agent.busy`. Archiving keeps everything. The cleanup paths and how an archived session is exempt:

- Terminal eviction (`max_exited`): an archived session's terminal is pinned as a retained one's is (decision 0090), and
  clearing a retain leaves it pinned while the session is archived. Clearing the archive unpins it unless retained.
- Startup: `mark_stale_on_startup` only records exits, and `settle_interrupted_starts` only removes run folders that have
  no session record, so neither touches an archived session.
- Explicit removals stay explicit: `agent.forget`, `agent.release` (which still archives the output and closes an
  archived session's terminal), `workspace.remove` (whose row deletion cascades to
  the workspace's sessions) and `project.remove --forget-workspaces` act on what the person names. No sweep removes a
  session, its terminal or its workspace on its own.

A resume clears the archive: a session run again is in use again, and `agent resume` says so when the session was
hidden.

**D4, delete.** `agent.forget` gains `remove_workspace`, `force`, `discard_ignored` and `expect_dirty`. With
`remove_workspace`, every check runs before anything changes, in this order: the forget's own (`agent.busy` for a live
run); that no other session (live, exited or archived) uses the workspace (`agent.busy` naming them); that no team or
task points at it and no terminal runs in it (`workspace.busy` naming them; teams and tasks have no foreign key, and a
running terminal is never stopped by this path); with `expect_dirty: false` (the preview showed it clean), that the
worktree has no uncommitted or untracked change now, even with `force` (`workspace.dirty`); then `workspace.remove`'s
refusals with `force` and `discard_ignored` as the waivers (`workspace.dirty`, `workspace.ignored_files`,
`workspace.main_worktree`, `workspace.locked`, `workspace.orphan`). `discard_ignored` deletes ignored files such as build
output and is passed only when the person chose it. The session is then forgotten. The removal begins under the start
gate and the workspace's mutation lock, where the other-users checks run again before the workspace is marked
`removing`; from then on starts, resumes and terminals in it are refused, so a session started between the first check
and the removal makes the removal refuse and keeps its record. A refusal at that point, after the forget, is returned
in `workspace_error`. The removal then finishes through `workspace.remove`'s own steps: its `workspace.removing` and
`workspace.removed` events, its worktree removal and its created-branch rule. A folder workspace is forgotten and its files stay, as `workspace.remove` does. The result is
the forgotten session with `workspace_removed` (the `workspace.remove` result), or `workspace_error { code, message }`
when the removal failed after the forget; the CLI prints that line. Without `remove_workspace` the call behaves as
before.

**D5, preview.** `agent.forget_preview { session }` returns `{ session, can_forget, forget_blocker?, workspace: { id,
kind, path?, branch?, removable, blocker?, blocker_code?, dirty, untracked, ignored_files?, other_sessions, other_teams?,
other_tasks?, running_terminals? } }`, read from the same checks without waivers. `removable` is false while anything
else uses the workspace or a removal without waivers would be refused; `blocker` and `blocker_code` say why. It changes
nothing. The path and the blocker messages go only to callers with `workspace` scope; a caller below it gets
`blocker_code` and a fixed message, since a refusal (a git error among them) can name paths.

**D6, rename.** `agent.rename { session, title }` sets a live or exited session's title, for the same session menu. The
title is trimmed and must be 1 to 120 characters (counted as characters) without control characters; otherwise
`request.invalid_params`, and nothing changes. Every reader gets the change as `agent.changed`.

**D7, wire and CLI.** Capability `agent-archive/1` covers D3 to D6; the client refuses the new calls and options before
sending to a service that does not offer it. CLI: `agent hide <SESSION> [--off]`, `agent list --archived`,
`agent forget <SESSION> [--remove-workspace [--force]]`, `agent forget <SESSION> --preview` and
`agent rename <SESSION> <TITLE>`. `agent archive` keeps
its meaning (the archived output of decision 0090).

## Consequences

- The picker reads order, icon, tone and offering from the declaration; a vendor that adds a mode needs only a
  declaration change.
- A delete that would lose another session's workspace, or uncommitted work without `force`, is refused before the
  session is forgotten.
- `agent.forget`'s result is the session with optional fields added, so an older caller still reads a session.
- A run start racing the delete between its checks and the removal is stopped by `workspace.remove`'s own stop pass, as
  for any workspace removal.
