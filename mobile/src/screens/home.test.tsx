import { fireEvent, render, screen } from '@testing-library/react-native';

import App from '../../App';
import { FakeMassOrca } from '../binding/fake';
import { agentSession, hostSummary, ProtocolRefusingFake } from '../state/test-fixtures';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

describe('Home host cards', () => {
  it('shows a connected host with its attention summary from the agent watch', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptAgentWatch([
      {
        tag: 'Item',
        item: {
          tag: 'Snapshot',
          sessions: [
            agentSession({ id: 's1', activityState: 'waiting' }),
            agentSession({ id: 's2', activityState: 'working' }),
            agentSession({ id: 's3', activityState: 'working' }),
          ],
        },
      },
    ]);

    await render(<App binding={fake} />);

    expect(await screen.findByText('Connected')).toBeTruthy();
    expect(await screen.findByText('1 session needs you, 2 working')).toBeTruthy();
  });

  it('marks a host that cannot be reached and retries it', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.failNext('connect', 'unreachable', "home-pc can't be reached.");

    await render(<App binding={fake} />);

    expect(await screen.findByText('Unreachable')).toBeTruthy();
    expect(screen.getByText("home-pc can't be reached.")).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));

    expect(await screen.findByText('Connected')).toBeTruthy();
  });

  it('keeps the last known state of a host whose link is lost, with its age, and never says stopped', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptAgentWatch([
      {
        tag: 'Item',
        item: {
          tag: 'Snapshot',
          sessions: [agentSession({ id: 's1', activityState: 'waiting' }), agentSession({ id: 's2', activityState: 'working' })],
        },
      },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 12_000n, reason: 'connection closed' } },
    ]);

    await render(<App binding={fake} />);

    expect(
      await screen.findByText('Last known state, a moment ago: 1 session needs you, 1 working. It may still be running.'),
    ).toBeTruthy();
    expect(screen.getByText('Unreachable')).toBeTruthy();
    expect(screen.queryByText('Stopped')).toBeNull();
  });

  it.each([
    ['not_paired', 'home-pc no longer trusts this phone.'],
    ['identity_mismatch', "home-pc's identity changed. Pair again."],
  ])('asks to pair again when the host does not trust this phone (%s)', async (code, message) => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.failNext('connect', code, message);

    await render(<App binding={fake} />);

    expect(await screen.findByText('Sign in')).toBeTruthy();
    expect(screen.getByText(message)).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Pair again' }));

    expect(await screen.findByLabelText('Pairing mode')).toBeTruthy();
  });

  it('sends a host that speaks another protocol to the protocol gate', async () => {
    await render(<App binding={new ProtocolRefusingFake({ min: 2, max: 3 })} />);

    expect(await screen.findByText('Update needed')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'home-pc, Update needed' }));

    expect(await screen.findByText('This app needs an update')).toBeTruthy();
  });
});
