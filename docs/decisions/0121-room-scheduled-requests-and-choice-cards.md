# 0121: Scheduled requests and choice cards in the Conference room

Status: accepted, 2026-11-25.

## Context

Decision 0102 made every person's line in a team's Conference room a task that starts at once. AUTO-13 and AUTO-28 ask
for more: a request with a day and a time becomes a one-time routine, and the room shows a "Routine created" card
(design audit C5). In pv96 one query on the default agent reads the day and time; this product does not run an agent
just to read a date.

## Decision

### Scheduled requests

- A person's line (not a line posted as a bot, and not an agent run's line) that names exactly one day and exactly one
  time becomes a one-time routine instead of a task. The reading is deterministic, in English and Spanish, with no
  agent call:
  - a day is `YYYY-MM-DD`, `today`/`hoy`, `tomorrow`/`mañana` (not "la mañana", the morning), or a weekday name, which
    means its next one after today;
  - a time is `HH:MM`, `H:MM am|pm`, `Ham|pm`, or a bare hour after `at` or `a las` that ends a clause (`at 9,`, `at 9`
    at the end, `at 9 on friday`). "at 3 places" is not a time.
  - Two different days, two different times, or a missing one leave the line a task, as before.
- Today is the date on the host's zone, the zone a routine without a zone runs on. The room sends no zone; the
  routine's zone is the host's, as a routine made in the desktop's form.
- The routine is made through the same path as `team.routine_add`, cadence `once`: its bot is the bot the line goes to
  (decision 0102's rules), its name the line's task title, its prompt the whole line, and it keeps the poster's
  host-admin scope for its run. No task is made.
- The bot answers "Routine created: {name}. It runs once on {date} at {time}." A moment the routine path refuses, such
  as one already past, is answered "Not scheduled: {reason}", and nothing is scheduled. The person's line is stored in
  both cases.
- The answer is a room line with the new optional column `routine_id`, a migration step that adds it to
  `team_room_messages` with `REFERENCES team_routines(id) ON DELETE SET NULL`: removing the routine keeps the line.
  `team.room_list` and `team.room_post` carry the routine on the line as the optional field `routine`, as
  `team.routine_list` shows it. Older clients ignore the field and still read the line's text.
- A scheduled line also publishes `team.changed` for the team, as `team.routine_add` does.
- The desktop draws the answer's "Routine created" card with the routine's name, when it runs, and "View routine",
  which opens the team's Routines page. The CLI prints the routine id and its date and time after the line.

## Not done here

- Repeating schedules from the room ("every Monday at 9"). They need a weekday or month-day reading this decision does
  not define; the Routines page makes them.
- A zone the poster names in the room.
- Asking before making a routine like one the same bot still has to run (pv96's similar-routine choice).

## Consequences

- A line that only mentions a date, such as a deadline with no time, still becomes a task.
- A time written as a bare hour with no clause end ("at 9 run the release") is not read as a time; "at 9, run the
  release" is. The rule prefers a missed schedule, which the person sees at once as a started task, over a wrong one.
