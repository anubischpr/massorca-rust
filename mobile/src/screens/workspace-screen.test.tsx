import { act, fireEvent, render, screen, waitFor } from '@testing-library/react-native';

import App from '../../App';
import { BindingProvider } from '../binding/binding-context';
import { FakeMassOrca } from '../binding/fake';
import { HostStoreProvider } from '../state/host-store-context';
import { hostSummary, terminalSummary, workspace } from '../state/test-fixtures';
import { webViews } from '../terminal/test-webview';
import { WorkspaceScreen } from './workspace-screen';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

const PROJECT_A = 'a1b2c3d4-0000-7000-8000-000000000001';

describe('the views of a workspace', () => {
  it('opens on Changes, and its Terminal view lists only the terminals of this workspace', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      terminals: [
        terminalSummary({ id: 't-1', title: 'build' }),
        terminalSummary({ id: 't-2', title: 'api' }),
        terminalSummary({ id: 't-3', title: 'other', workspaceId: 'ws-2' }),
      ],
    });
    await render(
      <BindingProvider binding={fake}>
        <HostStoreProvider binding={fake}>
          <WorkspaceScreen host="home-pc" workspace="ws-1" noGit={false} onOpenDetails={jest.fn()} onOpenTerminal={jest.fn()} />
        </HostStoreProvider>
      </BindingProvider>,
    );

    expect(await screen.findByText('No changes')).toBeTruthy();

    await fireEvent.press(screen.getByRole('tab', { name: 'Terminal' }));

    expect(await screen.findByText('Terminals (2)')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'build, Live, /repos/ws-1' })).toBeTruthy();
    expect(screen.queryByText('other')).toBeNull();
  });

  it('opens the one live terminal of a workspace from its Terminal view, through the navigator', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      workspaces: [workspace({ id: 'ws-1', projectId: PROJECT_A })],
      terminals: [terminalSummary({ id: 't-1', title: 'build' })],
    });
    await render(<App binding={fake} />);

    await fireEvent.press(await screen.findByRole('button', { name: 'home-pc, Connected' }));
    await fireEvent.press(await screen.findByRole('button', { name: 'ws-1' }));
    await fireEvent.press(await screen.findByRole('tab', { name: 'Terminal' }));

    expect(await screen.findByText('Loading terminal. Starting the shell on home-pc.')).toBeTruthy();
    await act(async () => {
      const page = webViews[webViews.length - 1];
      page?.post({ kind: 'ready', cols: 80, rows: 24 });
    });
    await waitFor(() => expect(fake.attaches).toEqual([{ terminal: 't-1', cols: 80, rows: 24 }]));
  });
});
