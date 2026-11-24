import { errorCode } from '../binding/ffi-error';
import type { FfiError, LinkState } from '../binding/types';

export interface LogLine {
  readonly atMs: number;
  readonly text: string;
}

/** X-DG-01 shows the last 50 lines. */
export const LOG_LINES = 50;

export function appendLogLine(log: readonly LogLine[], line: LogLine): readonly LogLine[] {
  return [...log, line].slice(-LOG_LINES);
}

export function linkLogText(link: LinkState): string {
  if (link.tag === 'Live') return 'Reconnected.';
  const seconds = Math.max(1, Math.round(Number(link.retryInMs) / 1000));
  return `Lost contact: ${link.reason}. Attempt ${link.attempt} in ${seconds} s.`;
}

export function errorLogText(error: FfiError): string {
  return `${errorCode(error)}: ${error.message}`;
}

/** Local wall-clock time, HH:MM:SS. */
export function formatLogClock(atMs: number): string {
  const date = new Date(atMs);
  return [date.getHours(), date.getMinutes(), date.getSeconds()]
    .map((part) => String(part).padStart(2, '0'))
    .join(':');
}
