# 0123: Automation definitions (AUTO-01, first version)

Status: accepted, 2026-11-25. The definitions are stored, listed and edited; runs are not started by this version.

## Context

AUTO-01 (docs/inventory/automations.md) asks for scheduled automations: a name, a prompt, an agent, a schedule, a time
zone, a project, and an enabled flag. Decision 0082 gives one core schedule engine for routines and automations. Its
claim, zone and occurrence rules are reused unchanged; this decision records what the engine gained and what the first
version leaves out.

## Decision

- D1 Schedule forms. Presets: hourly (every hour, at the minute of `time`), daily, weekdays (Monday to Friday) and
  weekly (one weekday). Plus a five-field cron expression. RRULE is not in this version and is deferred to a later
  change. The presets and cron compile into the same `CompiledSchedule` that routines use.
- D2 core-schedule. `Cadence` gains `Hourly` and `Weekdays`. An occurrence search considers every wall time of a day,
  so one cadence may fire several times a day. `Schedule` keeps its fields: adding one would change a literal in the
  routine server. Cron is a separate type, `CronSchedule` (a five-field expression and an optional zone).
  - Fields take numbers, lists, ranges, steps and month or weekday names. Day of week 7 is Sunday.
  - A step larger than its field's range is refused. The expression is at most 256 bytes.
  - An expression that can never occur is refused, such as 30 February or 31 April. 29 February is accepted and
    searched for over nine years, since the longest gap between leap days is eight years.
  - Day-of-month and day-of-week both restricted match a day when either matches, as standard cron reads them.
  - An expression with more than nine years between two of its days, such as `0 0 29 2 */7` (29 February that is a
    Sunday), is refused. The check walks the 400-year calendar cycle from every start, so it does not depend on the clock.
- D3 Store. Migration step `automations`: name, prompt, agent kind, cadence with its time, weekday or cron expression,
  an optional zone, an optional grace window, the enabled flag, an optional creation key, the next and last run times,
  and timestamps. CHECK constraints tie the weekday to the weekly cadence and the expression to the cron cadence.
  - `project_id` is set to NULL when its project is removed, so the automation remains and never starts. The orphan
    display belongs to AUTO-08.
  - `creation_key` is unique. A repeated create with the same key returns the automation the first create made.
  - A NULL zone means the host's zone, read when the schedule is used, as routines do. A create without a zone is
    refused when the host's zone cannot be read, because it would run at UTC without saying so.
  - A NULL grace means the default of 720 minutes. Its use belongs to AUTO-03. A grace is 0 to 10080 minutes (one week);
    the upper bound is this version's choice.
  - Runs have no table yet. Removing an automation therefore removes no run history.
- D4 Protocol. Capability `automations/1`. Methods `automation.create`, `automation.list`, `automation.show`,
  `automation.update`, `automation.rm`, `automation.enable` and `automation.disable`. A cadence or weekday this build
  does not know decodes as `Unknown`. Refusals: `automation.not_found`, `automation.schedule_invalid` (cadence, time,
  weekday, cron, grace) and `automation.zone_invalid`. An unknown agent kind is `agent.kind_not_found` and an unknown
  project is `project.not_found`, as elsewhere.
- D5 Server. Reads (`list`, `show`) need `read`; writes need `workspace`, as routines do. An automation is selected by
  id, then by exact name, and a name two automations share is refused until the id is given. A schedule change
  recomputes the next run; a name or prompt change keeps it. Enabling schedules the next run from now and checks the
  definition still holds; disabling clears the next run. A create with no project is refused.
  Reads show the next run from the clock at the time of the read, since no run advances the stored one yet; the stored
  next run is kept for AUTO-03.

## Desktop

- D6 Page. The Automations page (`docs/design/artboards/ws-automations.dc.html`, D-AG-14) lists each automation with
  its schedule, its project and its next run. A switch enables or disables it, and Delete asks before it removes the
  automation. Without a service the page says so and lists nothing; it shows no sample data.
- D7 Editor. The editor (D-AG-15) holds the fields of one form: name, project, prompt, agent, cadence with its time,
  weekday or cron expression, and zone. Cron sits in the same form in this version, not behind an advanced toggle.
  The form checks what it can before sending; the service checks the rest.

## CLI

- D8 Command. `massorca automations create|edit|show|list|remove|enable|disable`, with text output and `--json` like
  the other commands. Each command sends its `automation.*` method through the client; an older host that does not
  know the methods says so. `create` takes `--project` (an id, a name or a path), `--prompt`, `--agent` (default
  `claude`), `--cadence` (`hourly`, `daily`, `weekdays`, `weekly` or `cron`), `--time`, `--weekday`, `--cron`, `--zone`,
  `--grace` and `--creation-key`. `edit` takes the same options, each optional, and changes only those given.

## Not in this version

- Runs and run history (AUTO-03, AUTO-04), the precheck (AUTO-05), the agent launch and its arguments (AUTO-06),
  completion and usage (AUTO-07), host ownership (AUTO-08), destinations and workspace modes (AUTO-02), SSH and
  remote scheduling (AUTO-09), the calendar (AUTO-30).
- An events topic for automation changes. Clients re-read the list after a change.
