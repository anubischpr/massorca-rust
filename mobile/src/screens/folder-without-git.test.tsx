import { fireEvent, render, screen, waitFor } from '@testing-library/react-native';

import App from '../../App';
import { FakeMassOrca } from '../binding/fake';
import { hostSummary, workspace } from '../state/test-fixtures';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

const FOLDER = workspace({ id: 'ws-1', name: 'e2e-folder', kind: 'folder', branch: undefined });

/** Opens the host's Projects screen from Home, where the folder is listed with its No Git badge. */
async function openProjects(fake: FakeMassOrca) {
  await render(<App binding={fake} />);
  await fireEvent.press(await screen.findByRole('button', { name: 'home-pc, Connected' }));
}

describe('a folder without git', () => {
  it('opens its sheet from the No Git badge, which says what a plain folder lacks, and Not now closes it', async () => {
    await openProjects(new FakeMassOrca({ hosts: [hostSummary()], workspaces: [FOLDER] }));

    await fireEvent.press(await screen.findByTestId('no-git-badge'));

    expect(await screen.findByText('This folder is not a git repository')).toBeTruthy();
    expect(
      screen.getByText(
        'e2e-folder is a plain folder, so there are no branches or diffs to show. Initialize Git to track changes and use worktrees.',
      ),
    ).toBeTruthy();
    expect(screen.getByText('Needs MassOrca 0.9.4 or newer on This PC.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Initialize Git', disabled: true })).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Not now' }));

    await waitFor(() => expect(screen.queryByText('This folder is not a git repository')).toBeNull());
  });

  it('opens the same sheet from the TalkBack action of its row', async () => {
    await openProjects(new FakeMassOrca({ hosts: [hostSummary()], workspaces: [FOLDER] }));

    fireEvent(await screen.findByLabelText('e2e-folder, No Git'), 'accessibilityAction', { nativeEvent: { actionName: 'noGit' } });

    expect(await screen.findByText('This folder is not a git repository')).toBeTruthy();
  });

  it('shows no badge on a repository, so its row offers no folder action', async () => {
    await openProjects(new FakeMassOrca({ hosts: [hostSummary()], workspaces: [workspace({ id: 'ws-2', name: 'feature' })] }));

    expect(await screen.findByLabelText('feature')).toBeTruthy();
    expect(screen.queryByTestId('no-git-badge')).toBeNull();
    expect(screen.queryByText('No Git')).toBeNull();
  });
});
