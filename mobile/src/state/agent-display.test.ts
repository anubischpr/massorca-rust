import { aggregateDisplay, attentionCounts, sessionDisplay } from './agent-display';
import { agentSession } from './test-fixtures';

describe('sessionDisplay', () => {
  it('maps each live activity to its glyph and word', () => {
    expect(sessionDisplay(agentSession({ id: 's', activityState: 'working' }), false)).toEqual({
      tag: 'live',
      glyph: 'running',
      word: 'Working',
    });
    expect(sessionDisplay(agentSession({ id: 's', activityState: 'waiting' }), false)).toEqual({
      tag: 'live',
      glyph: 'waiting',
      word: 'Needs you',
    });
    expect(sessionDisplay(agentSession({ id: 's', activityState: 'idle' }), false)).toEqual({
      tag: 'live',
      glyph: 'idle',
      word: 'Idle',
    });
    expect(sessionDisplay(agentSession({ id: 's', activityState: 'done' }), false)).toEqual({
      tag: 'live',
      glyph: 'done',
      word: 'Done',
    });
  });

  it('shows no state before the agent has reported one', () => {
    expect(sessionDisplay(agentSession({ id: 's', activityState: undefined }), false)).toBeUndefined();
  });

  it('shows an exited run as done', () => {
    expect(sessionDisplay(agentSession({ id: 's', executionState: 'exited', activityState: 'working' }), false)).toEqual({
      tag: 'live',
      glyph: 'done',
      word: 'Done',
    });
  });

  it('shows an unverifiable run as unreachable with its last known state, never as stopped', () => {
    const display = sessionDisplay(agentSession({ id: 's', executionState: 'unverifiable', activityState: 'working' }), false);

    expect(display).toEqual({ tag: 'unreachable', lastKnown: 'running' });
  });

  it('treats a lost link as unreachable for every session', () => {
    const display = sessionDisplay(agentSession({ id: 's', activityState: 'waiting' }), true);

    expect(display).toEqual({ tag: 'unreachable', lastKnown: 'waiting' });
  });

  it('treats an execution state it does not know as unreachable', () => {
    const session = agentSession({ id: 's', executionState: 'stopped', activityState: undefined });

    expect(sessionDisplay(session, false)).toEqual({ tag: 'unreachable', lastKnown: undefined });
  });
});

describe('aggregateDisplay', () => {
  it('shows the most urgent session of a row', () => {
    const sessions = [
      agentSession({ id: 'a', activityState: 'idle' }),
      agentSession({ id: 'b', activityState: 'working' }),
      agentSession({ id: 'c', activityState: 'waiting' }),
    ];

    expect(aggregateDisplay(sessions, false)).toEqual({ tag: 'live', glyph: 'waiting', word: 'Needs you' });
  });

  it('shows nothing for a row without sessions', () => {
    expect(aggregateDisplay([], false)).toBeUndefined();
  });

  it('shows the most urgent last known state when the link is lost', () => {
    const sessions = [agentSession({ id: 'a', activityState: 'idle' }), agentSession({ id: 'b', activityState: 'working' })];

    expect(aggregateDisplay(sessions, true)).toEqual({ tag: 'unreachable', lastKnown: 'running' });
  });
});

describe('attentionCounts', () => {
  const sessions = [
    agentSession({ id: 'a', activityState: 'waiting' }),
    agentSession({ id: 'b', activityState: 'working' }),
    agentSession({ id: 'c', activityState: 'working' }),
  ];

  it('counts sessions that need the user and sessions working', () => {
    expect(attentionCounts(sessions, false)).toEqual({ needsYou: 1, working: 2 });
  });

  it('counts the last known states when the link is lost', () => {
    expect(attentionCounts(sessions, true)).toEqual({ needsYou: 1, working: 2 });
  });
});
