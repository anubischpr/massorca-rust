# 0122: Conversation replies and the typing line in the Conference room

Status: accepted, 2026-11-25.

## Context

AUTO-13 asks that a conversation get "one answer from one bot, under its own name, of at most 1,200 characters", that
"bots post in the room as themselves", that "a reply to a quoted message keeps the quoted line as its context", that
"the room shows a typing line while a bot works", and that "a reply task is not listed as a task there". Decisions 0102
and 0121 make every person's line a task, a routine or a choice card. In pv96 one query on the default agent decides
whether a line is a conversation, and a one-shot run of the answering bot's agent writes the answer
(`bot-team-conversation.ts`, `REPLY_MAX_CHARS = 1200`). This product runs no agent just to classify a line.

## Decision

### When a line is a conversation

- A person's line is a conversation, not a task, when `team.room_post` sets `ask: true`, or when it sets `reply_to` to
  a line of a bot of this team (a bot line that names its bot and is not a choice card) and does not set `ask: false`.
  Every other line goes on as decisions 0102 and 0121 say. There is no classifier: the person says it, by asking or by
  answering a bot.
- `reply_to` names a line of the same team's room; any other id is `team.room_line_not_found`. The person's line keeps
  it (the new column `reply_to`), and `team.room_list` shows it with the optional field `quoted` (the quoted line's
  id, author, bot name and its text cut to 200 characters).
- A task made from a line with `reply_to` keeps the quoted line as context: its body is the line, then
  "In reply to {author}: {quoted text}". Its title is still the line's own first line. A task asked in reply to a
  bot's line (`ask: false`) goes to that bot unless the line names another with `@name`, ahead of `to`, the lead and
  the only bot, so it gets no choice card.
- The bot that answers is the bot the line names with `@name`, else the bot whose line it replies to, else `to`, else
  the lead, else the only bot, else the team's first bot. A question never gets a choice card.

### The answer

- The answering bot gets a reply run: its agent kind starts in the team's folder workspace, through the same path and
  permission mode as its tasks (decision 0072), with one prompt: who it is, the room's last 20 lines, the quoted line,
  and the person's line, told to answer once, as itself, in at most 1,200 characters, and to create no task, change no
  file and run no command. No task is made, so the Board never lists a reply.
- A reply run starts without vault variables, whatever the caller's scope: a conversation needs none, and so never
  gets more than the person who asked (decision 0071).
- The answer is posted once, as that bot, in one of two ways, whichever comes first: the bot posts with
  `team room post` from inside its run (its session and run token, as a task's run posts), or the run's turn ends and
  the host posts the turn's result text. The person's line holds the reply's state (`open`, `answered`, `closed`), and
  each move is one statement that changes only the state it expects, so two answers never post. A bot run posts only
  as its own reply's bot, and a second line from it is refused.
- The answer is cut to 1,200 characters (1,199 and "…"). A turn that ends with no result, a failed or stopped turn, or
  a run the host forgot posts "I could not answer this time.". A run that does not start posts "I could not answer: "
  and the reason at once. The answer line's `reply_to` names the person's line.
- When the turn ends, the host stops the reply run, so no idle run is left per question.

### The typing line

- `team.room_list` carries the optional field `typing`: each bot of the team whose reply run is open and still working,
  or whose room task is working, once, in team order. The desktop draws "{bot} is typing…" under the thread, and the
  CLI prints it after the lines of `team room list`.
- The room publishes `team.room_changed` when a reply starts and when it is answered, as for a task's start and its
  outcome, and the desktop reads the room again on it.

### Wire and storage

- One migration step adds `reply_to`, `reply_bot_id`, `reply_session` and `reply_state` to `team_room_messages`.
- `ask`, `reply_to`, `quoted` and `typing` are optional and additive. A host that honours them advertises
  `team-room/3` and keeps `team-room/1` and `team-room/2`. An older host ignores `ask` and `reply_to` and makes a task,
  so the CLI refuses `--ask` and `--reply-to` without `team-room/3`, and the desktop offers Reply and Just ask only on
  a host that has it.
- The CLI: `team room post <team> <text> --ask` asks without making a task, `--reply-to <message>` quotes a line, and
  `--task` makes a reply to a bot's line a task. The desktop: a Reply button on each bot line, the quoted line above
  the composer, and a Just ask checkbox.

## Not done here

- pv96's query on the default agent that tells a conversation from a task, greetings answered by All, research and
  code-reading answers, and the rotation of the answering bot by usage.
- The quoted line of a request that gets a choice card is not carried into the task the card's answer makes.
- A room task that waits for the person keeps its bot on the typing line until the room changes again.

## Consequences

- A question with no `ask` and no reply to a bot still becomes a task, as before.
- A reply costs one agent turn of the answering bot's kind, in the team's folder.
