export const CONNECTION_LOG_TITLE = 'Connection log';
export const RETRY = 'Retry';
export const RETRY_NOW = 'Retry now';
export const DETAILS = 'Details';
export const PAIR_AGAIN = 'Pair again';

/** X-CN-01 banner copy; the host name is the one the phone gave the host. */
export const BANNER = {
  connecting: (host: string) => `Connecting to ${host}`,
  unreachable: (host: string, age: string | undefined) =>
    age === undefined
      ? `${host} can't be reached. Work that was running there may still be running.`
      : `${host} can't be reached. Showing the last known state from ${age}. Work that was running there may still be running.`,
  reconnecting: (host: string, attempt: number, seconds: number) =>
    `Reconnecting to ${host}, attempt ${attempt}. Retrying in ${seconds} s.`,
  restored: (host: string) => `Reconnected to ${host}.`,
  signIn: (host: string) => `${host} needs you to sign in`,
  failed: (host: string, message: string) => `Could not connect to ${host}. ${message}`,
} as const;

/** X-DG-01 status line words. */
export const STATUS_WORDS = { ok: 'Ok', warning: 'Warning', failed: 'Failed', connecting: 'Connecting' } as const;

export const LOG_HEADING = 'Log';
export const LOG_EMPTY = 'Nothing has been logged yet.';
export const WHAT_TO_TRY = 'What to try';
export const TRY_STEPS = (host: string): readonly string[] => [`Check that ${host} is on`, 'Check the network'];
export const NO_CONTACT = 'No contact yet';
export const lastContactLine = (age: string): string => `Last contact ${age}`;
