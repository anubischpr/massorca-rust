import type { TerminalEvent } from '../binding/types';
import { applyTerminalEvent, INITIAL_TERMINAL_SESSION, type TerminalSession } from './terminal-session';

const NOW = 1_000_000;

const snapshot: TerminalEvent = { tag: 'Snapshot', offset: 0n, cols: 80, rows: 24, bytes: new ArrayBuffer(0) };
const output: TerminalEvent = { tag: 'Output', offset: 0n, bytes: new ArrayBuffer(0) };
const lost: TerminalEvent = {
  tag: 'Link',
  link: { tag: 'Unverifiable', attempt: 1, retryInMs: 2000n, reason: 'connection closed' },
};
const live: TerminalEvent = { tag: 'Link', link: { tag: 'Live' } };

/** Folds the events in, each one `stepMs` after the one before, from `NOW`. */
function play(events: readonly TerminalEvent[], stepMs = 0): TerminalSession {
  return events.reduce(
    (session, event, index) => applyTerminalEvent(session, event, NOW + index * stepMs),
    INITIAL_TERMINAL_SESSION,
  );
}

describe('applyTerminalEvent', () => {
  it('attaches until the first snapshot opens the view, and then shows the process running', () => {
    expect(INITIAL_TERMINAL_SESSION.phase.tag).toBe('attaching');

    const session = play([snapshot]);

    expect(session.phase.tag).toBe('attached');
    expect(session.restoredAtMs).toBeUndefined();
    expect(session.lastContactMs).toBe(NOW);
  });

  it('restores the screen when a later snapshot replaces output the view missed', () => {
    const session = play([output, snapshot], 1_000);

    expect(session.phase.tag).toBe('attached');
    expect(session.restoredAtMs).toBe(NOW + 1_000);
  });

  it('keeps the last contact while the link is lost, and restores the screen when it comes back', () => {
    const lostSession = play([snapshot, lost], 1_000);
    expect(lostSession.link.tag).toBe('Unverifiable');
    expect(lostSession.lastContactMs).toBe(NOW);

    const back = applyTerminalEvent(lostSession, live, NOW + 5_000);
    expect(back.link).toEqual({ tag: 'Live' });
    expect(back.restoredAtMs).toBe(NOW + 5_000);
    expect(back.lastContactMs).toBe(NOW + 5_000);
  });

  it('records the code of an exit, or its signal when there is no code', () => {
    const coded = play([snapshot, { tag: 'Exited', reason: 'exited', code: 1, signal: undefined }]);
    expect(coded.phase).toEqual({ tag: 'exited', reason: 'exited', code: 1, signal: undefined });

    const signalled = play([snapshot, { tag: 'Exited', reason: 'killed', code: undefined, signal: 'SIGKILL' }]);
    expect(signalled.phase).toEqual({ tag: 'exited', reason: 'killed', code: undefined, signal: 'SIGKILL' });
  });

  it('counts a resize as contact without changing the process', () => {
    const session = play([snapshot, { tag: 'Resized', cols: 100, rows: 30 }], 2_000);

    expect(session.phase.tag).toBe('attached');
    expect(session.lastContactMs).toBe(NOW + 2_000);
  });
});
