import type { TerminalEvent } from '../binding/types';
import { INITIAL_LINK, touchContact, trackLink, type LinkTracker } from '../state/link-tracker';

/** Where the process stands as the view shows it. An attach that failed is decided by the screen, not here. */
export type TerminalPhase =
  | { readonly tag: 'attaching' }
  | { readonly tag: 'attached' }
  | {
      readonly tag: 'exited';
      readonly reason: string;
      readonly code: number | undefined;
      readonly signal: string | undefined;
    };

export interface TerminalSession extends LinkTracker {
  readonly phase: TerminalPhase;
}

export const INITIAL_TERMINAL_SESSION: TerminalSession = { ...INITIAL_LINK, phase: { tag: 'attaching' } };

/** Folds one step of a terminal view into what the screen shows. */
export function applyTerminalEvent(session: TerminalSession, event: TerminalEvent, nowMs: number): TerminalSession {
  switch (event.tag) {
    case 'Output':
      return { ...touchContact(session, nowMs), phase: { tag: 'attached' } };
    case 'Snapshot': {
      // The first snapshot opens the view; a later one replaces output the view missed, so the screen was restored.
      const restored = session.phase.tag === 'attached';
      return {
        ...touchContact(session, nowMs),
        phase: { tag: 'attached' },
        restoredAtMs: restored ? nowMs : session.restoredAtMs,
      };
    }
    case 'Resized':
      return touchContact(session, nowMs);
    case 'Exited':
      return {
        ...session,
        phase: { tag: 'exited', reason: event.reason, code: event.code, signal: event.signal },
      };
    case 'Link':
      return { ...session, ...trackLink(session, event.link, nowMs) };
  }
}
