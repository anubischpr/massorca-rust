import { appendLogLine, errorLogText, formatLogClock, linkLogText, LOG_LINES, type LogLine } from './connection-log';

describe('appendLogLine', () => {
  it('keeps only the last 50 lines', () => {
    let log: readonly LogLine[] = [];
    for (let index = 0; index < LOG_LINES + 5; index += 1) {
      log = appendLogLine(log, { atMs: index, text: `line ${index}` });
    }

    expect(log).toHaveLength(LOG_LINES);
    expect(log[0]?.text).toBe('line 5');
    expect(log[LOG_LINES - 1]?.text).toBe(`line ${LOG_LINES + 4}`);
  });
});

describe('linkLogText', () => {
  it('says the link came back', () => {
    expect(linkLogText({ tag: 'Live' })).toBe('Reconnected.');
  });

  it('gives the reason, the attempt and the wait in whole seconds', () => {
    expect(linkLogText({ tag: 'Unverifiable', attempt: 3, retryInMs: 12_400n, reason: 'connection closed' })).toBe(
      'Lost contact: connection closed. Attempt 3 in 12 s.',
    );
  });

  it('never shows a wait below one second', () => {
    expect(linkLogText({ tag: 'Unverifiable', attempt: 1, retryInMs: 0n, reason: 'refused' })).toBe(
      'Lost contact: refused. Attempt 1 in 1 s.',
    );
  });
});

describe('errorLogText', () => {
  it('starts with the stable code, then the sentence', () => {
    expect(errorLogText({ tag: 'Unreachable', message: "home-pc can't be reached." })).toBe(
      "unreachable: home-pc can't be reached.",
    );
  });
});

describe('formatLogClock', () => {
  it('formats local time as HH:MM:SS with leading zeros', () => {
    expect(formatLogClock(new Date(2026, 10, 24, 9, 5, 7).getTime())).toBe('09:05:07');
  });
});
