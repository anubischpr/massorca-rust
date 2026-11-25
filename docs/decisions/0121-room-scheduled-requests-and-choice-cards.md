# 0121: Scheduled requests and choice cards in the Conference room

Status: accepted, 2026-11-25.

## Context

Decision 0102 made every person's line in a team's Conference room a task that starts at once. AUTO-13 and AUTO-28 ask
for more: a request with a day and a time becomes a one-time routine, and the room shows a "Routine created" card
(design audit C5). In pv96 one query on the default agent reads the day and time; this product does not run an agent
just to read a date.

## Decision

### Scheduled requests

- A person's line (not a line posted as a bot, and not an agent run's line) that names exactly one day and asks for
  exactly one time becomes a one-time routine instead of a task. The reading is deterministic, in English and Spanish,
  with no agent call:
  - a day is `YYYY-MM-DD`, `today`/`hoy`, `tomorrow`/`mañana`, or a weekday name, which means its next one after today.
    A possessive is not a day ("tomorrow's standup"), "mañana" after `la`, `esta`, `de`, `en`, `por` or `pasado` is not
    tomorrow ("esta mañana" is this morning), and "after tomorrow" is not tomorrow;
  - a time is asked for when it follows `at` or `a las`, or when the line says schedule or remind (`schedule`,
    `remind`, `programa`, `programar`, `recuerda`, `recuérdame`, `recordar`). A time that is only mentioned ("the 5pm
    deploy today will break login") is not asked for;
  - a time is `HH:MM`, `H:MM am|pm`, `Ham|pm`, or a bare hour after `at` or `a las` that ends a clause or comes before
    a day (`at 17,`, `at 17 on friday`). A bare hour from 1 to 11 says neither morning nor evening, so `at 3` and
    `a las 5` are not read; 0 and 12 to 23 are. For the same reason `H:MM` with one hour digit from 1 to 9 and no
    am/pm (`at 3:30`) is not read, while `03:30` and `3:30 pm` are. A line with such an unclear time is not scheduled
    even when it also names a clear one;
  - two different days, two different times, or a missing one leave the line a task, as before.
- Today is the date on the host's zone (UTC when the host's zone cannot be read, as core-schedule reads it), the zone
  a routine without a zone runs on. The room sends no zone; the routine's zone is the host's, as a routine made in the
  desktop's form, and the room's card names it.
- The routine is made through the same path as `team.routine_add`, cadence `once`: its bot is the bot the line goes to
  (decision 0102's rules), its name the line's task title, its prompt the whole line, and it keeps the poster's
  host-admin scope for its run. No task is made.
- The bot answers "Routine created: {name}. It runs once on {date} at {time}." A routine the path refuses, such as a
  moment already past or a host whose zone cannot be read, stores nothing, and the line becomes a task as any other
  line, so nothing the person wrote is lost.
- The answer is a room line with the new optional column `routine_id`, a migration step that adds it to
  `team_room_messages` with `REFERENCES team_routines(id) ON DELETE SET NULL`: removing the routine keeps the line.
  `team.room_list` and `team.room_post` carry the routine on the line as the optional field `routine`, as
  `team.routine_list` shows it. Older clients ignore the field and still read the line's text.
- A scheduled line also publishes `team.changed` for the team, as `team.routine_add` does.
- The desktop draws the answer's "Routine created" card with the routine's name, when it runs and on which zone, and
  "View routine", which opens the team's Routines page. The CLI prints the routine id and its date and time after
  the line.

### A room task's title and start

- A room task's title is the line's first line that is not blank once the `@name` of each of the team's bots is left
  out (at the start of a word, the longest name first), with its spaces collapsed and the punctuation a removed mention
  leaves at its start, cut to 80 characters. "@Mara, parse the headers" makes "parse the headers". The body keeps the
  whole line as written.
- `team.room_post` takes the optional `start`. `false` leaves a person's task in Todo (the Board's Open column) and the
  bot answers "Added to the Board, not started: {title}"; absent or `true` starts it at once, as before. The CLI flag
  is `--no-start`, and the desktop composer has a Start at once checkbox, on by default. Older hosts ignore the field
  and start the task.

## Not done here

- Repeating schedules from the room ("every Monday at 9"). They need a weekday or month-day reading this decision does
  not define; the Routines page makes them.
- A zone the poster names in the room.
- Asking before making a routine like one the same bot still has to run (pv96's similar-routine choice).

## Consequences

- A line that only mentions a date, such as a deadline with no time, still becomes a task.
- A time written as a bare morning-or-evening hour ("at 9"), or one only mentioned, is not read as a time. The rule
  prefers a missed schedule, which the person sees at once as a started task, over a wrong one.
