import type { RoutineRunSummary, RoutineSummary } from '../binding/types';
import {
  FINISHED_WORD,
  LAST_RUN,
  LAST_RUN_FAILED,
  NEXT_WORD,
  NO_BOT_WORD,
  NO_RUNS,
  PAUSED_WORD,
  SCHEDULE,
  WEEKDAY_NAMES,
} from '../copy/routines';
import { formatAge, formatUntil } from '../state/relative-age';

/** A routine with what its row needs: its bot's name, and its newest run when it has run at all. */
export interface RoutineEntry {
  readonly routine: RoutineSummary;
  /** Absent when the routine's bot is gone. */
  readonly botName: string | undefined;
  readonly lastRun: RoutineRunSummary | undefined;
}

/** The routines of one team, under the team's name. */
export interface RoutineSection {
  readonly teamId: string;
  readonly teamName: string;
  readonly routines: readonly RoutineEntry[];
}

/** The words one routine row shows. */
export interface RoutineRowText {
  readonly title: string;
  /** For example "Every Monday at 09:00, mara, next in 4 days". */
  readonly summary: string;
  /** "Last run 2 h ago", "Last run failed 2 h ago" or "No runs yet". */
  readonly lastRun: string;
  /** The sentence of a failed run, when its run failed. */
  readonly lastRunError: string | undefined;
  readonly paused: boolean;
}

/** When a routine runs, as its row reads it: "Every Monday at 09:00". */
export function scheduleText(routine: RoutineSummary): string {
  const time = routine.time;
  switch (routine.cadence) {
    case 'daily':
      return SCHEDULE.daily(time);
    case 'weekly': {
      const day = weekdayName(routine.weekday);
      return day === undefined ? SCHEDULE.unknownDay(time) : SCHEDULE.weekly(day, time);
    }
    case 'monthly':
      return routine.monthDay === undefined ? SCHEDULE.unknownDay(time) : SCHEDULE.monthly(routine.monthDay, time);
    case 'once':
      return routine.runDate === undefined ? SCHEDULE.unknownDay(time) : SCHEDULE.once(routine.runDate, time);
    default:
      return SCHEDULE.unknownDay(time);
  }
}

/** Whether the routine is paused, has its next run, or will not run again. */
function statusText(routine: RoutineSummary, nowMs: number): string {
  if (routine.paused) return PAUSED_WORD;
  if (routine.nextRunAtMs === undefined) return FINISHED_WORD;
  return NEXT_WORD(formatUntil(Number(routine.nextRunAtMs) - nowMs));
}

function lastRunText(run: RoutineRunSummary | undefined, nowMs: number): string {
  if (run === undefined) return NO_RUNS;
  const age = formatAge(nowMs - Number(run.createdAtMs));
  return run.error === undefined ? LAST_RUN(age) : LAST_RUN_FAILED(age);
}

/** The words of one routine row at `nowMs`. */
export function routineRowText(entry: RoutineEntry, nowMs: number): RoutineRowText {
  const { routine, botName, lastRun } = entry;
  return {
    title: routine.name,
    summary: [scheduleText(routine), botName ?? NO_BOT_WORD, statusText(routine, nowMs)].join(', '),
    lastRun: lastRunText(lastRun, nowMs),
    lastRunError: lastRun?.error,
    paused: routine.paused,
  };
}

/** The sections with `updated` in place of the routine that has its id; the other routines keep their entries. */
export function replaceRoutine(sections: readonly RoutineSection[], updated: RoutineSummary): RoutineSection[] {
  return sections.map((section) => ({
    ...section,
    routines: section.routines.map((entry) => (entry.routine.id === updated.id ? { ...entry, routine: updated } : entry)),
  }));
}

/** The routine that `routineId` names among the sections, if any. */
export function findRoutine(sections: readonly RoutineSection[], routineId: string): RoutineEntry | undefined {
  for (const section of sections) {
    const entry = section.routines.find((candidate) => candidate.routine.id === routineId);
    if (entry !== undefined) return entry;
  }
  return undefined;
}

function weekdayName(weekday: string | undefined): string | undefined {
  return weekday === undefined ? undefined : WEEKDAY_NAMES[weekday];
}
