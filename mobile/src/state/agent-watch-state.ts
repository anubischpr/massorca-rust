import type { AgentEvent, AgentSessionSummary } from '../binding/types';
import { INITIAL_LINK, touchContact, trackLink, type LinkTracker } from './link-tracker';

export interface AgentWatchState extends LinkTracker {
  readonly sessions: readonly AgentSessionSummary[];
}

export const INITIAL_AGENT_WATCH: AgentWatchState = { ...INITIAL_LINK, sessions: [] };

/** Folds one event of the agent watch into the state the screens show. */
export function applyAgentEvent(state: AgentWatchState, event: AgentEvent, nowMs: number): AgentWatchState {
  if (event.tag === 'Item') {
    const { item } = event;
    const sessions = item.tag === 'Snapshot' ? item.sessions : withSession(state.sessions, item.session);
    return { ...touchContact(state, nowMs), sessions };
  }
  return { ...state, ...trackLink(state, event.link, nowMs) };
}

function withSession(
  sessions: readonly AgentSessionSummary[],
  session: AgentSessionSummary,
): readonly AgentSessionSummary[] {
  const exists = sessions.some((known) => known.id === session.id);
  return exists
    ? sessions.map((known) => (known.id === session.id ? session : known))
    : [...sessions, session];
}
