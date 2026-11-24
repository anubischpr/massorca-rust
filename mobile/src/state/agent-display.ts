import type { AgentSessionSummary } from '../binding/types';
import { AGENT_WORDS } from '../copy/agent';

export type LiveGlyph = 'running' | 'waiting' | 'idle' | 'done';

/** What a row shows: a live state, or no contact so only the last known state is left. */
export type AgentDisplay =
  | { readonly tag: 'live'; readonly glyph: LiveGlyph; readonly word: string }
  | { readonly tag: 'unreachable'; readonly lastKnown: LiveGlyph | undefined };

/** "Needs you" outranks working; unreachable ranks between them, ordered by its last known state. */
const LIVE_RANK: Record<LiveGlyph, number> = { waiting: 4, running: 3, idle: 1, done: 0 };
const UNREACHABLE_RANK = 2;
const KNOWN_EXECUTION = new Set(['live', 'exited']);

/** Loss of contact (the link or the host's own verdict) is never shown as stopped: it is unreachable. */
export function sessionDisplay(session: AgentSessionSummary, linkLost: boolean): AgentDisplay | undefined {
  const lastKnown = lastKnownGlyph(session);
  if (linkLost || !KNOWN_EXECUTION.has(session.executionState)) return { tag: 'unreachable', lastKnown };
  return lastKnown === undefined ? undefined : liveDisplay(lastKnown);
}

/** The one state a row shows for all its sessions: the most urgent one. */
export function aggregateDisplay(sessions: readonly AgentSessionSummary[], linkLost: boolean): AgentDisplay | undefined {
  let best: AgentDisplay | undefined;
  for (const session of sessions) {
    const display = sessionDisplay(session, linkLost);
    if (display !== undefined && (best === undefined || rank(display) > rank(best))) best = display;
  }
  return best;
}

export function attentionCounts(
  sessions: readonly AgentSessionSummary[],
  linkLost: boolean,
): { readonly needsYou: number; readonly working: number } {
  let needsYou = 0;
  let working = 0;
  for (const session of sessions) {
    const display = sessionDisplay(session, linkLost);
    const glyph = display === undefined ? undefined : display.tag === 'live' ? display.glyph : display.lastKnown;
    if (glyph === 'waiting') needsYou += 1;
    if (glyph === 'running') working += 1;
  }
  return { needsYou, working };
}

function rank(display: AgentDisplay): number {
  if (display.tag === 'live') return LIVE_RANK[display.glyph];
  return UNREACHABLE_RANK + (display.lastKnown === undefined ? 0 : LIVE_RANK[display.lastKnown] / 10);
}

function lastKnownGlyph(session: AgentSessionSummary): LiveGlyph | undefined {
  if (session.executionState === 'exited') return 'done';
  switch (session.activityState) {
    case 'working':
      return 'running';
    case 'waiting':
      return 'waiting';
    case 'idle':
      return 'idle';
    case 'done':
      return 'done';
    default:
      return undefined;
  }
}

function liveDisplay(glyph: LiveGlyph): AgentDisplay {
  return { tag: 'live', glyph, word: AGENT_WORDS[glyph] };
}
