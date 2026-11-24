import { fireEvent, render, screen } from '@testing-library/react-native';

import App from '../../App';
import { FakeMassOrca } from '../binding/fake';
import { agentSession, hostSummary, StalledWorkspacesFake, workspace } from '../state/test-fixtures';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

const PROJECT_A = 'a1b2c3d4-0000-7000-8000-000000000001';
const PROJECT_B = 'b1b2c3d4-0000-7000-8000-000000000002';

/** Opens the host's card with the badge it shows, which leads to its projects. */
async function openProjects(fake: FakeMassOrca, badge: string) {
  await render(<App binding={fake} />);
  await fireEvent.press(await screen.findByRole('button', { name: `home-pc, ${badge}` }));
}

describe('workspaces of a host', () => {
  it('groups them by project, marks plain folders No Git, and shows each row its agent state', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      workspaces: [
        workspace({ id: 'ws-1', projectId: PROJECT_A }),
        workspace({ id: 'ws-2', projectId: PROJECT_A, kind: 'folder', branch: undefined }),
        workspace({ id: 'ws-3', projectId: PROJECT_B }),
      ],
    });
    fake.scriptAgentWatch([
      {
        tag: 'Item',
        item: {
          tag: 'Snapshot',
          sessions: [
            agentSession({ id: 's1', workspaceId: 'ws-1', activityState: 'working' }),
            agentSession({ id: 's2', workspaceId: 'ws-2', activityState: 'waiting' }),
          ],
        },
      },
    ]);

    await openProjects(fake, 'Connected');

    expect(await screen.findByText('Project a1b2c3d4')).toBeTruthy();
    expect(screen.getByText('Project b1b2c3d4')).toBeTruthy();
    expect(screen.getByLabelText('ws-1, Working')).toBeTruthy();
    expect(screen.getByLabelText('ws-2, Needs you, No Git')).toBeTruthy();
    expect(screen.getByLabelText('ws-3')).toBeTruthy();
    expect(screen.getByText('No Git')).toBeTruthy();
  });

  it('says there are no workspaces yet when the host has none', async () => {
    await openProjects(new FakeMassOrca({ hosts: [hostSummary()] }), 'Connected');

    expect(await screen.findByText('No workspaces yet')).toBeTruthy();
    expect(screen.getByText('A workspace is a git worktree or a folder where an agent works.')).toBeTruthy();
  });

  it('shows a loading state until the list arrives', async () => {
    await openProjects(new StalledWorkspacesFake({ hosts: [hostSummary()] }), 'Connected');

    expect(await screen.findByLabelText('Loading workspaces')).toBeTruthy();
  });

  it('shows an error when the list cannot be read, and retries it', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], workspaces: [workspace({ id: 'ws-1' })] });
    fake.failNext('workspaces', 'internal', 'Unexpected data from the host.');
    await openProjects(fake, 'Connected');

    expect(await screen.findByText('Could not load workspaces from home-pc')).toBeTruthy();
    expect(screen.getByText('Unexpected data from the host.')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));

    expect(await screen.findByLabelText('ws-1')).toBeTruthy();
  });

  it('keeps the last known state of each row, dimmed, when the link is lost', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], workspaces: [workspace({ id: 'ws-1' })] });
    fake.scriptAgentWatch([
      {
        tag: 'Item',
        item: { tag: 'Snapshot', sessions: [agentSession({ id: 's1', workspaceId: 'ws-1', activityState: 'working' })] },
      },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 12_000n, reason: 'connection closed' } },
    ]);
    await openProjects(fake, 'Unreachable');

    expect(
      await screen.findByText(
        "home-pc can't be reached. Showing the last known state from a moment ago. Work that was running there may still be running.",
      ),
    ).toBeTruthy();
    expect(screen.getByLabelText('ws-1, Last known: Working, a moment ago')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Retry now' })).toBeTruthy();
  });

  it('confirms when the link comes back after a loss', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], workspaces: [workspace({ id: 'ws-1' })] });
    fake.scriptAgentWatch([
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [] } },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 12_000n, reason: 'connection closed' } },
      { tag: 'Link', link: { tag: 'Live' } },
    ]);
    await render(<App binding={fake} />);

    await fireEvent.press(await screen.findByRole('button', { name: /^home-pc, / }));

    expect(await screen.findByText('Reconnected to home-pc.')).toBeTruthy();
  });

  it('says how far a reconnect has got and when the next attempt runs', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], workspaces: [workspace({ id: 'ws-1' })] });
    fake.scriptAgentWatch([
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [] } },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 2, retryInMs: 12_000n, reason: 'connection closed' } },
    ]);
    await openProjects(fake, 'Unreachable');

    expect(await screen.findByText('Reconnecting to home-pc, attempt 2. Retrying in 12 s.')).toBeTruthy();
  });

  it('shows a host that cannot be reached with its reason, and retries the connection', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], workspaces: [workspace({ id: 'ws-1' })] });
    // One refusal for the attempt from Home, one for the attempt that opening the projects makes.
    fake.failNext('connect', 'unreachable', "home-pc can't be reached.");
    fake.failNext('connect', 'unreachable', "home-pc can't be reached.");
    await openProjects(fake, 'Unreachable');

    expect(await screen.findByText('Could not load workspaces from home-pc')).toBeTruthy();
    expect(screen.getByText('Work that was running there may still be running.', { exact: false })).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));

    expect(await screen.findByLabelText('ws-1')).toBeTruthy();
  });
});
