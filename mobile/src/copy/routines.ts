// Copy of the Routines screen: the host's routines by team (AUTO-28; docs/design/screens/android.md, section 11.1).

export const ROUTINES_TITLE = 'Routines';
/** The entry to the routines, on the host's Projects screen: the phone has no team screen yet. */
export const ROUTINES_ACTION = 'Routines';
export const LOADING_ROUTINES = 'Loading routines';
export const NO_ROUTINES_TITLE = 'No routines yet';
export const NO_ROUTINES_LINE = 'A routine starts a bot on a task at a time you set. Make one on This PC.';
export const ROUTINES_NOT_SUPPORTED_TITLE = 'Routines are not on This PC';
export const ROUTINES_NOT_SUPPORTED_LINE = 'Routines need a newer MassOrca on This PC.';
export const routinesLoadFailed = (host: string): string => `Could not load routines from ${host}`;
export const NOT_CHANGED = (reason: string): string => `Not changed: ${reason}`;

export const PAUSED_WORD = 'paused';
export const FINISHED_WORD = 'finished';
export const NEXT_WORD = (when: string): string => `next ${when}`;
export const NO_BOT_WORD = 'no bot';
export const NO_RUNS = 'No runs yet';
export const LAST_RUN = (age: string): string => `Last run ${age}`;
export const LAST_RUN_FAILED = (age: string): string => `Last run failed ${age}`;

/** How each schedule reads on its row. `time` is 24-hour `HH:MM`. */
export const SCHEDULE = {
  daily: (time: string): string => `Every day at ${time}`,
  weekly: (day: string, time: string): string => `Every ${day} at ${time}`,
  monthly: (day: number, time: string): string => `On day ${day} of each month at ${time}`,
  once: (date: string, time: string): string => `Once on ${date} at ${time}`,
  unknownDay: (time: string): string => `At ${time}`,
};

export const WEEKDAY_NAMES: Readonly<Record<string, string>> = {
  monday: 'Monday',
  tuesday: 'Tuesday',
  wednesday: 'Wednesday',
  thursday: 'Thursday',
  friday: 'Friday',
  saturday: 'Saturday',
  sunday: 'Sunday',
};
