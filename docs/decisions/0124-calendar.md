# 0124: Calendar of routines and automations (AUTO-30, first version)

Status: accepted, 2026-11-25. The Calendar shows the runs of routines and automations in one week. Done runs show on
request, and the Show done switch is kept for each viewer.

## Context

AUTO-30 (docs/inventory/automations.md) asks for one Calendar on the activity rail that shows every routine and
automation on one timeline. Routines and automations are one core schedule engine with two presentations (decisions
0082 and 0123), so the Calendar reads that engine. A routine run is placed on its routine's zone.

## Decision

- D1 Protocol. Capability `calendar/1`, method `calendar.occurrences`, with read permission like the routine and
  automation lists. The window runs from `from_ms`, inclusive, to `to_ms`, exclusive, in UTC milliseconds, and spans at
  most 62 days; a reversed or longer window is refused with `calendar.window_invalid`. `kind` (`routine` or
  `automation`) is optional, and `include_done` is false unless asked. The result lists at most 2,000 runs, earliest
  first, and sets `truncated` when a window holds more.
- D2 Upcoming runs. A run at or after the time of the call is upcoming. A routine's upcoming runs come from the shared
  engine; an automation's come from its schedule through the same engine. A paused routine and a disabled automation
  have none. Automations keep no run history yet (AUTO-03), so they show upcoming runs only.
- D3 Done runs. A routine run from its history is done once its outcome is recorded; a run still pending is not done.
  Done runs appear only with `include_done`. The store has no window query, so the history is read newest first, up to
  10,000 runs per routine; a window older than those runs shows none of them.
- D4 Placement. Each run carries `at_ms` and its wall clock on its zone: `local_date` (`YYYY-MM-DD`), `local_time`
  (`HH:MM`) and `zone`, the routine's or automation's zone, or the host's zone name when it has none. A routine made on
  the Routines page and one made by a bot in the Conference room are the same stored rows, so both appear.
- D5 Core schedule. `massorca-core-schedule` gains `occurrences_between` and `wall_clock_in`, in a new module. The
  occurrence search and the cron rules of decision 0123 are unchanged.
- D6 Server. Routines are read from the store, and their schedules come from `teams::routines::schedule_of`, the mapping
  the Bot Teams routine methods use too. Automations are read through their feature's own module, with their compiled
  schedule and the automation view.

## Desktop

- D7 Page. The Calendar is a full page opened from the activity rail. It has Previous, Today and Next, the week's
  range title, and the Show done switch. Each day is a column with its runs in time order; an empty day reads
  "Nothing scheduled", and an empty week shows the empty state. A click on a routine opens its team's Routines page,
  and a click on an automation opens the Automations page. The Show done switch is saved for this viewer in the
  desktop's `ui-state.json` and read at start, so the page opens as it was left; it is off until it is switched on.
- D8 Modes. The chips are All, Team (Bot Teams routines) and Automations (Workspaces automations), in the words of the
  Calendar artboard (`docs/design/artboards/bt-calendar-week.dc.html`).
- D9 Older services. A service without `calendar/1` is refused with `service.capability_missing`. The page says the
  service does not offer the calendar yet; it does not fail.

## CLI

- D10 Command. `massorca calendar [--from <DATE>] [--days <N>] [--kind routines|automations] [--done]`, with text and
  `--json` output. `--from` is a UTC date, today in UTC when unset. `--days` is 1 to 62, 7 when unset.

## Gaps

- Day and Month views, and the hour grid that places runs on the artboard. The page lists each day's runs in time
  order.
- The artboard's team picker is not built. The Team chip filters by kind.
- Automation runs and their done state wait for AUTO-03.

## Amendment (decision 0125, 2026-11-26)

- Automations keep a run history now. With `include_done`, an automation's completed runs show as done at the
  occurrence they stand for, read from its kept history (the newest 100 final runs); failed, missed and skipped runs
  do not (D2, D3).
