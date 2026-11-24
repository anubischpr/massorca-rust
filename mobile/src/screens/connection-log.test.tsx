import { fireEvent, render, screen } from '@testing-library/react-native';

import App from '../../App';
import { FakeMassOrca } from '../binding/fake';
import { agentSession, hostSummary, workspace } from '../state/test-fixtures';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

describe('connection log', () => {
  it('records a lost link with its reason and the next attempt, and shows the status as a warning', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], workspaces: [workspace({ id: 'ws-1' })] });
    fake.scriptAgentWatch([
      {
        tag: 'Item',
        item: { tag: 'Snapshot', sessions: [agentSession({ id: 's1', workspaceId: 'ws-1', activityState: 'working' })] },
      },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 12_000n, reason: 'connection closed' } },
    ]);
    await render(<App binding={fake} />);

    await fireEvent.press(await screen.findByRole('button', { name: 'home-pc, Unreachable' }));
    await fireEvent.press(await screen.findByRole('button', { name: 'Details' }));

    expect(await screen.findByText('Lost contact: connection closed. Attempt 1 in 12 s.', { exact: false })).toBeTruthy();
    expect(screen.getByText('Warning')).toBeTruthy();
    expect(screen.getByText('1. Check that home-pc is on')).toBeTruthy();
    expect(screen.getByText('2. Check the network')).toBeTruthy();
  });

  it('records a connect that failed, with its stable code, and shows the status as failed', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    // One refusal for the attempt from Home, one for the attempt that opening the projects makes.
    fake.failNext('connect', 'unreachable', "home-pc can't be reached.");
    fake.failNext('connect', 'unreachable', "home-pc can't be reached.");
    await render(<App binding={fake} />);

    await fireEvent.press(await screen.findByRole('button', { name: 'home-pc, Unreachable' }));
    await fireEvent.press(await screen.findByRole('button', { name: 'Details' }));

    expect((await screen.findAllByText("unreachable: home-pc can't be reached.", { exact: false })).length).toBeGreaterThan(0);
    expect(screen.getByText('Failed')).toBeTruthy();
  });
});
