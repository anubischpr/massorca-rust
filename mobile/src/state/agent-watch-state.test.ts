import type { AgentEvent, LinkState } from '../binding/types';
import { applyAgentEvent, INITIAL_AGENT_WATCH } from './agent-watch-state';
import { agentSession } from './test-fixtures';

const lostLink: LinkState = { tag: 'Unverifiable', attempt: 1, retryInMs: 12_000n, reason: 'connection closed' };
const unverifiable: AgentEvent = { tag: 'Link', link: lostLink };
const live: AgentEvent = { tag: 'Link', link: { tag: 'Live' } };

describe('applyAgentEvent', () => {
  it('replaces the sessions with a snapshot and records the contact time', () => {
    const first = agentSession({ id: 's1' });
    const state = applyAgentEvent(
      INITIAL_AGENT_WATCH,
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [first] } },
      1_000,
    );

    expect(state.sessions).toEqual([first]);
    expect(state.lastContactMs).toBe(1_000);
  });

  it('updates a changed session in place and appends a new one', () => {
    const working = agentSession({ id: 's1', activityState: 'working' });
    const snapshot = applyAgentEvent(
      INITIAL_AGENT_WATCH,
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [working] } },
      1_000,
    );
    const done = agentSession({ id: 's1', activityState: 'done' });
    const other = agentSession({ id: 's2' });

    const afterChange = applyAgentEvent(snapshot, { tag: 'Item', item: { tag: 'Changed', session: done } }, 2_000);
    const afterNew = applyAgentEvent(afterChange, { tag: 'Item', item: { tag: 'Changed', session: other } }, 3_000);

    expect(afterNew.sessions).toEqual([done, other]);
  });

  it('keeps the last known sessions and contact time when the link is lost', () => {
    const working = agentSession({ id: 's1', activityState: 'working' });
    const snapshot = applyAgentEvent(
      INITIAL_AGENT_WATCH,
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [working] } },
      1_000,
    );

    const lost = applyAgentEvent(snapshot, unverifiable, 9_000);

    expect(lost.link).toEqual(lostLink);
    expect(lost.sessions).toEqual([working]);
    expect(lost.lastContactMs).toBe(1_000);
    expect(lost.restoredAtMs).toBeUndefined();
  });

  it('marks when the link came back after a loss', () => {
    const lost = applyAgentEvent(INITIAL_AGENT_WATCH, unverifiable, 1_000);
    const back = applyAgentEvent(lost, live, 5_000);

    expect(back.link).toEqual({ tag: 'Live' });
    expect(back.lastContactMs).toBe(5_000);
    expect(back.restoredAtMs).toBe(5_000);
  });

  it('does not mark a restore when the link was never lost', () => {
    expect(applyAgentEvent(INITIAL_AGENT_WATCH, live, 1_000).restoredAtMs).toBeUndefined();
  });
});
