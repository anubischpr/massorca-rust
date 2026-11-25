import { fireEvent, render, screen, waitFor } from '@testing-library/react-native';

import { BindingProvider } from '../binding/binding-context';
import { FakeMassOrca } from '../binding/fake';
import { LOADING_TERMINALS, NO_TERMINAL_TITLE, TERMINALS_FAILED_TITLE } from '../copy/terminal';
import { HostStoreProvider } from '../state/host-store-context';
import { agentSession, hostSummary, StalledTerminalsFake, terminalSummary } from '../state/test-fixtures';
import { TerminalListScreen } from './terminal-list-screen';

// The first render of the view loads its state on a loaded machine; a cold run needs more than the default.
jest.setTimeout(20_000);

const LOST_LINK = "home-pc can't be reached. Showing the last known state from a moment ago. Work that was running there may still be running.";

async function renderList(fake: FakeMassOrca) {
  const onOpenDetails = jest.fn();
  const onOpenTerminal = jest.fn();
  await render(
    <BindingProvider binding={fake}>
      <HostStoreProvider binding={fake}>
        <TerminalListScreen host="home-pc" workspace="ws-1" onOpenDetails={onOpenDetails} onOpenTerminal={onOpenTerminal} />
      </HostStoreProvider>
    </BindingProvider>,
  );
  return { onOpenDetails, onOpenTerminal };
}

describe('terminals of a workspace', () => {
  it('shows that it is loading until the list arrives', async () => {
    await renderList(new StalledTerminalsFake({ hosts: [hostSummary()] }));

    expect(await screen.findByLabelText(LOADING_TERMINALS)).toBeTruthy();
  });

  it('opens the only live terminal of the workspace at once, and no other', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      terminals: [terminalSummary({ id: 't-1', title: 'build' }), terminalSummary({ id: 't-9', workspaceId: 'ws-2' })],
    });
    const { onOpenTerminal } = await renderList(fake);

    await waitFor(() => expect(onOpenTerminal).toHaveBeenCalledWith('t-1'));
    expect(onOpenTerminal).toHaveBeenCalledTimes(1);
  });

  it('lists several terminals with their name, folder and state word, and opens the one tapped', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      terminals: [
        terminalSummary({ id: 't-1', title: 'build' }),
        terminalSummary({ id: 't-2', title: undefined, shell: 'zsh', cwd: '/repos/ws-1/api' }),
        terminalSummary({ id: 't-3', title: 'old run', state: 'exited' }),
      ],
    });
    const { onOpenTerminal } = await renderList(fake);

    expect(await screen.findByRole('button', { name: 'build, Live, /repos/ws-1' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'zsh, Live, /repos/ws-1/api' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'old run, Exited, /repos/ws-1' })).toBeTruthy();
    expect(screen.getByText('Terminals (3)')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'zsh, Live, /repos/ws-1/api' }));

    expect(onOpenTerminal).toHaveBeenCalledTimes(1);
    expect(onOpenTerminal).toHaveBeenCalledWith('t-2');
  });

  it('says there is no terminal in the workspace when it has none', async () => {
    const { onOpenTerminal } = await renderList(new FakeMassOrca({ hosts: [hostSummary()] }));

    expect(await screen.findByText(NO_TERMINAL_TITLE)).toBeTruthy();
    expect(onOpenTerminal).not.toHaveBeenCalled();
  });

  it('shows why the list cannot be read, and reads it again on Retry', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      terminals: [terminalSummary({ id: 't-1' }), terminalSummary({ id: 't-2', title: 'api' })],
    });
    fake.failNext('terminals', 'internal', 'Unexpected data from the host.');
    await renderList(fake);

    expect(await screen.findByText(TERMINALS_FAILED_TITLE)).toBeTruthy();
    expect(screen.getByText('Unexpected data from the host.')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));

    expect(await screen.findByText('Terminals (2)')).toBeTruthy();
  });

  it('keeps the last list, dimmed, with the age of the last contact, while the link is lost', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      terminals: [terminalSummary({ id: 't-1', title: 'build' }), terminalSummary({ id: 't-2', title: 'api' })],
    });
    fake.scriptAgentWatch([
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [agentSession({ id: 's1', workspaceId: 'ws-1' })] } },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 12_000n, reason: 'connection closed' } },
    ]);
    await renderList(fake);

    expect(await screen.findByText(LOST_LINK)).toBeTruthy();
    expect(screen.getByText('Terminals (2)')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'build, Live, /repos/ws-1' })).toBeTruthy();
    expect(screen.queryByText(/stopped/i)).toBeNull();
  });

  it('reads the list again when the link comes back', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      terminals: [terminalSummary({ id: 't-1', title: 'build' }), terminalSummary({ id: 't-2', title: 'api' })],
    });
    fake.scriptAgentWatch([
      { tag: 'Item', item: { tag: 'Snapshot', sessions: [] } },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 12_000n, reason: 'connection closed' } },
      { tag: 'Link', link: { tag: 'Live' } },
    ]);
    await renderList(fake);

    expect(await screen.findByText('Reconnected to home-pc.')).toBeTruthy();
    await waitFor(() => expect(fake.calls.filter((call) => call === 'terminals')).toHaveLength(2));
    expect(screen.getByText('Terminals (2)')).toBeTruthy();
  });
});
