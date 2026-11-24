import { FakeMassOrca } from '../binding/fake';
import type { AgentEvent } from '../binding/types';
import { HostStore } from './host-store';
import { agentSession, hostSummary } from './test-fixtures';

const unverifiable: AgentEvent = {
  tag: 'Link',
  link: { tag: 'Unverifiable', attempt: 1, retryInMs: 12_000n, reason: 'connection closed' },
};
const live: AgentEvent = { tag: 'Link', link: { tag: 'Live' } };

function setup() {
  const fake = new FakeMassOrca({ hosts: [hostSummary()] });
  const store = new HostStore(fake, () => 1_000);
  return { fake, store };
}

/** Lets the store's background work run until the check holds, or fails the test. */
async function until(check: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (check()) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  throw new Error('the store did not reach the expected state');
}

describe('HostStore', () => {
  it('connects a paired host and logs the connection', async () => {
    const { store } = setup();

    await store.ensure('home-pc');

    const record = store.getRecord('home-pc');
    expect(record?.connection.status).toBe('connected');
    expect(record?.log.map((line) => line.text)).toEqual(['Connecting to home-pc', 'Connected to home-pc']);
  });

  it('records a failed connect with its stable code', async () => {
    const { fake, store } = setup();
    fake.failNext('connect', 'unreachable', "home-pc can't be reached.");

    await store.ensure('home-pc');

    const record = store.getRecord('home-pc');
    expect(record?.connection).toEqual({
      status: 'failed',
      error: { tag: 'Unreachable', message: "home-pc can't be reached." },
    });
    expect(record?.log.at(-1)?.text).toBe("unreachable: home-pc can't be reached.");
  });

  it('does not connect again while a connection is open', async () => {
    const { fake, store } = setup();

    await store.ensure('home-pc');
    await store.ensure('home-pc');

    expect(fake.calls.filter((call) => call === 'connect')).toHaveLength(1);
  });

  it('connects again on retry after a failure', async () => {
    const { fake, store } = setup();
    fake.failNext('connect', 'unreachable', 'down');

    await store.ensure('home-pc');
    await store.ensure('home-pc');

    expect(store.getRecord('home-pc')?.connection.status).toBe('connected');
  });

  it('folds the agent watch into the record and logs a lost link with its reason', async () => {
    const { fake, store } = setup();
    fake.scriptAgentWatch([
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [agentSession({ id: 's1', activityState: 'working' })] } },
      unverifiable,
    ]);

    await store.ensure('home-pc');
    await until(() => store.getRecord('home-pc')?.agents?.link.tag === 'Unverifiable');

    const record = store.getRecord('home-pc');
    expect(record?.agents?.sessions.map((s) => s.id)).toEqual(['s1']);
    expect(record?.agents?.lastContactMs).toBe(1_000);
    expect(record?.log.at(-1)?.text).toBe('Lost contact: connection closed. Attempt 1 in 12 s.');
  });

  it('logs the return of the link', async () => {
    const { fake, store } = setup();
    fake.scriptAgentWatch([unverifiable, live]);

    await store.ensure('home-pc');
    await until(() => store.getRecord('home-pc')?.agents?.restoredAtMs !== undefined);

    expect(store.getRecord('home-pc')?.log.at(-1)?.text).toBe('Reconnected.');
  });

  it('keeps the last known agent state when a reconnect fails', async () => {
    const { fake, store } = setup();
    fake.scriptAgentWatch([{ tag: 'Item', item: { tag: 'Snapshot', sessions: [agentSession({ id: 's1' })] } }]);
    await store.ensure('home-pc');
    await until(() => store.getRecord('home-pc')?.agents?.sessions.length === 1);

    fake.failNext('connect', 'unreachable', 'down');
    await store.retry('home-pc');

    const record = store.getRecord('home-pc');
    expect(record?.connection.status).toBe('failed');
    expect(record?.agents?.sessions.map((s) => s.id)).toEqual(['s1']);
  });

  it('notifies subscribers when a record changes', async () => {
    const { store } = setup();
    let notifications = 0;
    const unsubscribe = store.subscribe(() => {
      notifications += 1;
    });

    await store.ensure('home-pc');
    unsubscribe();

    expect(notifications).toBeGreaterThan(0);
  });
});
