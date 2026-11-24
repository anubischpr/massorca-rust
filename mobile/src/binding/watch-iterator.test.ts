import { asyncIterate } from './watch-iterator';
import { FakeMassOrca } from './fake';
import type { AgentEvent, AgentSessionSummary, Watch } from './types';

const session: AgentSessionSummary = {
  id: 'session-1',
  kind: 'claude',
  workspaceId: 'workspace-1',
  title: undefined,
  activityState: 'working',
  executionState: 'running',
  updatedAtMs: 1n,
};

const snapshot: AgentEvent = { tag: 'Item', item: { tag: 'Snapshot', sessions: [session] } };
const changed: AgentEvent = { tag: 'Item', item: { tag: 'Changed', session: { ...session, executionState: 'exited' } } };
const unverifiable: AgentEvent = {
  tag: 'Link',
  link: { tag: 'Unverifiable', attempt: 1, retryInMs: 2000n, reason: 'connection closed' },
};
const live: AgentEvent = { tag: 'Link', link: { tag: 'Live' } };

/** A watch that records how it was closed, so a test can see whether the iterator released it. */
class RecordingWatch<E> implements Watch<E> {
  closedCount = 0;
  private readonly queue: E[];

  constructor(events: E[]) {
    this.queue = events;
  }

  async next(): Promise<E | null> {
    return this.queue.shift() ?? null;
  }

  close(): void {
    this.closedCount += 1;
  }
}

describe('asyncIterate', () => {
  it('yields events in order and ends when the watch returns null', async () => {
    const watch = new RecordingWatch<AgentEvent>([snapshot, unverifiable, live, changed]);
    const seen: AgentEvent[] = [];
    for await (const event of asyncIterate(watch)) seen.push(event);

    expect(seen).toEqual([snapshot, unverifiable, live, changed]);
    expect(watch.closedCount).toBe(1);
  });

  it('closes the watch when the consumer stops early (break calls return)', async () => {
    const watch = new RecordingWatch<AgentEvent>([snapshot, changed]);
    const iterator = asyncIterate(watch);

    expect((await iterator.next()).value).toEqual(snapshot);
    await iterator.return(undefined);

    expect(watch.closedCount).toBe(1);
  });

  it('closes the watch and rethrows when next fails', async () => {
    const failing: Watch<AgentEvent> = {
      next: () => Promise.reject(new Error('stream broke')),
      close: jest.fn(),
    };

    await expect(asyncIterate(failing).next()).rejects.toThrow('stream broke');
    expect(failing.close).toHaveBeenCalledTimes(1);
  });

  it('passes Link events from a scripted fake host through unchanged', async () => {
    const fake = new FakeMassOrca();
    fake.scriptAgentWatch([snapshot, unverifiable, live]);
    const host = await fake.connect(await addHost(fake));

    const seen: AgentEvent[] = [];
    for await (const event of asyncIterate(host.watchAgents())) seen.push(event);

    expect(seen).toEqual([snapshot, unverifiable, live]);
  });
});

async function addHost(fake: FakeMassOrca): Promise<string> {
  const host = await fake.pair('fake-offer:test', 'home-pc');
  return host.name;
}
