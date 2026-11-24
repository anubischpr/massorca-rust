import type { AgentEvent, AgentSessionSummary, LinkState } from '../binding/types';

export interface AgentWatchState {
  readonly sessions: readonly AgentSessionSummary[];
  readonly link: LinkState;
  /** Phone clock: when the host last answered. Ages are measured from here, never from the host's clock. */
  readonly lastContactMs: number | undefined;
  /** Phone clock: when the link came back after a loss. */
  readonly restoredAtMs: number | undefined;
}

export const INITIAL_AGENT_WATCH: AgentWatchState = {
  sessions: [],
  link: { tag: 'Live' },
  lastContactMs: undefined,
  restoredAtMs: undefined,
};

/** Folds one event of the agent watch into the state the screens show. */
export function applyAgentEvent(state: AgentWatchState, event: AgentEvent, nowMs: number): AgentWatchState {
  if (event.tag === 'Item') {
    const { item } = event;
    const sessions = item.tag === 'Snapshot' ? item.sessions : withSession(state.sessions, item.session);
    return { ...state, sessions, lastContactMs: nowMs };
  }
  const { link } = event;
  if (link.tag === 'Unverifiable') return { ...state, link };
  const wasLost = state.link.tag === 'Unverifiable';
  return { ...state, link, lastContactMs: nowMs, restoredAtMs: wasLost ? nowMs : state.restoredAtMs };
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
