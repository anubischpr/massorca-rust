import type { RoutineRunSummary, RoutineSummary } from '../binding/types';
import { replaceRoutine, routineRowText, scheduleText, type RoutineEntry, type RoutineSection } from './routines-model';

const NOW = 1_800_000_000_000;
const HOUR = 3_600_000;

function routine(overrides: Partial<RoutineSummary> & { readonly id: string }): RoutineSummary {
  return {
    teamId: 't-1',
    botId: 'b-1',
    name: 'morning',
    cadence: 'daily',
    time: '09:00',
    weekday: undefined,
    monthDay: undefined,
    runDate: undefined,
    zone: undefined,
    paused: false,
    nextRunAtMs: BigInt(NOW + 4 * 86_400_000),
    createdAtMs: 1n,
    ...overrides,
  };
}

function run(overrides: Partial<RoutineRunSummary> = {}): RoutineRunSummary {
  return {
    id: 'run-1',
    routineId: 'r-1',
    scheduledForMs: BigInt(NOW - 2 * HOUR),
    taskId: 'task-1',
    error: undefined,
    errorCode: undefined,
    createdAtMs: BigInt(NOW - 2 * HOUR),
    ...overrides,
  };
}

function entry(routineValue: RoutineSummary, overrides: Partial<RoutineEntry> = {}): RoutineEntry {
  return { routine: routineValue, botName: 'mara', lastRun: undefined, ...overrides };
}

describe('scheduleText', () => {
  it('reads each cadence with its day and time', () => {
    expect(scheduleText(routine({ id: 'r-1', cadence: 'daily' }))).toBe('Every day at 09:00');
    expect(scheduleText(routine({ id: 'r-1', cadence: 'weekly', weekday: 'monday' }))).toBe('Every Monday at 09:00');
    expect(scheduleText(routine({ id: 'r-1', cadence: 'monthly', monthDay: 28, time: '23:59' }))).toBe(
      'On day 28 of each month at 23:59',
    );
    expect(scheduleText(routine({ id: 'r-1', cadence: 'once', runDate: '2026-12-01' }))).toBe(
      'Once on 2026-12-01 at 09:00',
    );
  });

  it('falls back to the time alone when a day is missing or unknown', () => {
    expect(scheduleText(routine({ id: 'r-1', cadence: 'weekly', weekday: 'unknown' }))).toBe('At 09:00');
    expect(scheduleText(routine({ id: 'r-1', cadence: 'monthly' }))).toBe('At 09:00');
    expect(scheduleText(routine({ id: 'r-1', cadence: 'once' }))).toBe('At 09:00');
    expect(scheduleText(routine({ id: 'r-1', cadence: 'unknown' }))).toBe('At 09:00');
  });
});

describe('routineRowText', () => {
  it('reads the schedule, the bot and when the next run is', () => {
    const text = routineRowText(entry(routine({ id: 'r-1', cadence: 'weekly', weekday: 'monday' })), NOW);

    expect(text).toEqual({
      title: 'morning',
      summary: 'Every Monday at 09:00, mara, next in 4 days',
      lastRun: 'No runs yet',
      lastRunError: undefined,
      paused: false,
    });
  });

  it('says a paused routine is paused, with no next run', () => {
    const text = routineRowText(entry(routine({ id: 'r-1', paused: true, nextRunAtMs: undefined })), NOW);

    expect(text.summary).toBe('Every day at 09:00, mara, paused');
    expect(text.paused).toBe(true);
  });

  it('says a one-time routine that has run is finished, and names a bot that is gone', () => {
    const text = routineRowText(
      entry(routine({ id: 'r-1', cadence: 'once', runDate: '2026-11-01', nextRunAtMs: undefined }), { botName: undefined }),
      NOW,
    );

    expect(text.summary).toBe('Once on 2026-11-01 at 09:00, no bot, finished');
  });

  it('shows the age of the newest run, and the sentence of a run that failed', () => {
    const ok = routineRowText(entry(routine({ id: 'r-1' }), { lastRun: run() }), NOW);
    expect(ok.lastRun).toBe('Last run 2 h ago');
    expect(ok.lastRunError).toBeUndefined();

    const failed = routineRowText(
      entry(routine({ id: 'r-1' }), {
        lastRun: run({ taskId: undefined, error: 'bot removed: the routine has no bot to run', errorCode: 'bot.not_found' }),
      }),
      NOW,
    );
    expect(failed.lastRun).toBe('Last run failed 2 h ago');
    expect(failed.lastRunError).toBe('bot removed: the routine has no bot to run');
  });
});

describe('replaceRoutine', () => {
  const sections: RoutineSection[] = [
    {
      teamId: 't-1',
      teamName: 'alpha',
      routines: [entry(routine({ id: 'r-1' })), entry(routine({ id: 'r-2', name: 'evening' }))],
    },
  ];

  it('puts the updated routine in place and keeps the others and their runs', () => {
    const lastRun = run();
    const withRun: RoutineSection[] = [{ ...sections[0]!, routines: [entry(routine({ id: 'r-1' }), { lastRun })] }];
    const updated = routine({ id: 'r-1', paused: true, nextRunAtMs: undefined });

    const next = replaceRoutine(withRun, updated);

    expect(next[0]?.routines[0]).toEqual({ routine: updated, botName: 'mara', lastRun });
    expect(next[0]?.teamName).toBe('alpha');
  });

  it('leaves the sections alone when no routine has the id', () => {
    expect(replaceRoutine(sections, routine({ id: 'r-9' }))).toEqual(sections);
  });
});
