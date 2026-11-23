# 0082 Core schedule engine

Status: accepted, 2026-11-23.

## Context

The owner's direction is one core scheduling engine with two presentations: Automations in Workspaces mode and
Routines in Bot Teams mode (docs/inventory/automations.md, AUTO-01, AUTO-03, AUTO-28). Routines are its first user;
automations reuse it unchanged. The reviewed shape: occurrence rules, an IANA zone with a host fallback, an
exactly-once claim by compare-and-swap, only the latest missed run, and nothing made up after a pause. The service
also runs on Android (0080), which has no system tzdata directory that a library could read as files.

## Decision

- D1 Crate. `massorca-core-schedule` (`crates/core-schedule`) holds the schedule shape, its validation, the
  occurrence searches and the claim decision. It has no store, server, teams, bots, tasks or automation types, and no
  database dependency. Instants cross its API as UTC epoch milliseconds (`i64`).
- D2 Schedule shape. A cadence (daily, weekly on a weekday, monthly on day 1 to 28, once on a `YYYY-MM-DD` date), a
  24-hour `HH:MM` wall time, and an optional IANA zone name of at most 64 characters. Validation returns a typed
  error per mistake: weekly without a weekday, monthly without a day or with a day outside 1 to 28, once without a
  date, a bad date, a bad time, an unknown zone, a zone name that is too long. Day 29 to 31 is refused so every
  month has the day.
- D3 Zone library: `jiff` 1.0, with default features off and `std`, `tz-system` and `tzdb-bundle-always` on.
  - Bundled data: `tzdb-bundle-always` compiles the IANA database into the binary (`jiff-tzdb`, about 200 KB of
    data) on every target, so Android, Windows and Linux resolve the same names with the same rules; only host zone
    detection (D4) reads a system setting. `tzdb-zoneinfo` and `tzdb-concatenated` are off for the same reason.
  - Build weight: three new crates (`jiff`, `jiff-core`, `jiff-tzdb`; `windows-link` was already in the lock), no
    build script and no code generation. `chrono` + `chrono-tz` was the alternative: `chrono-tz` generates its tables
    in a build script from `parse-zoneinfo` and `phf_codegen`, which is heavier to build, and its daylight-saving
    handling (`LocalResult`) leaves gap resolution to the caller.
  - Licence: `jiff`, `jiff-core` and `jiff-tzdb` are `Unlicense OR MIT`; the zone data is public domain.
  - Not yet proven: no Android target was installed when this was written, so the Android build is unchecked. Its
    zone detection path exists in `jiff` (below).
- D4 Host zone. A schedule without a zone uses the host's zone, read when the schedule is compiled through
  `jiff::tz::TimeZone::try_system`: the `TZ` variable first, then `/etc/localtime` on Linux and macOS,
  `GetDynamicTimeZoneInformation` mapped to an IANA name through CLDR data on Windows, and the
  `persist.sys.timezone` property on Android. When none can be read, UTC is used and the compiled schedule reports
  `ZoneSource::HostUnknownUsedUtc`, so a caller can show that the time is UTC.
- D5 Daylight saving. A wall time that does not exist fires at the end of the gap (02:30 on a night that jumps from
  02:00 to 03:00 fires at 03:00 local time). A wall time that happens twice fires once, at the earlier instant. Tests
  cover both in America/New_York and Europe/Madrid.
- D6 Bounded searches. A search walks at most 370 local days and stops at the end of representable time. An instant
  outside that range has no occurrence.
- D7 Claim. `decide_claim(schedule, next_run_at, now)` is pure. A `None` `next_run_at` (paused or finished) or one
  after `now` is not due. Otherwise the run stands for the latest occurrence at or before `now`; earlier missed
  occurrences are dropped and counted (capped at 1000); the new `next_run_at` is the first occurrence after `now`,
  `None` for a finished one-time schedule. If the schedule has no occurrence since the stored time (it was edited
  without recomputing), the stored time runs once.
- D8 Exactly once. The caller's store makes the claim with a compare-and-swap before the run starts:
  `UPDATE ... SET next_run_at = :new WHERE id = :id AND next_run_at = :old`. One changed row owns the run; none means
  another caller claimed it. Because `next_run_at` has already moved past `now`, a run that cannot start records why
  and is not retried by the next check.
- D9 Resume. Creating or resuming a schedule writes `next_run_from(schedule, now)`, the first occurrence after now.
  Runs missed while paused are never made up.

## Consequences

- Routines (next) and automations store `next_run_at` as an integer column and call this crate; neither keeps its own
  occurrence code.
- The cron and RRULE schedule forms of AUTO-01 and its grace window are not in this crate yet; adding them extends
  `Cadence` and the occurrence walk, not the claim.
- Zone data updates arrive with `jiff-tzdb` releases, through ordinary dependency updates.
