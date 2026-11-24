import { act, fireEvent, render, screen, waitFor } from '@testing-library/react-native';

import { BindingProvider } from '../binding/binding-context';
import { FakeMassOrca, type FakeTerminalView } from '../binding/fake';
import { HostStoreProvider } from '../state/host-store-context';
import { agentSession, hostSummary } from '../state/test-fixtures';
import { webViews } from '../terminal/test-webview';
import { TerminalScreen } from './terminal-screen';

// A cold run on a loaded machine needs more than the default five seconds for the screen and its page.
jest.setTimeout(20_000);

const snapshot = { tag: 'Snapshot' as const, offset: 0n, cols: 80, rows: 24, bytes: new TextEncoder().encode('$ ').buffer };

async function renderScreen(fake: FakeMassOrca, workspace?: string) {
  const onOpenDetails = jest.fn();
  await render(
    <BindingProvider binding={fake}>
      <HostStoreProvider binding={fake}>
        <TerminalScreen host="home-pc" terminal="term-1" workspace={workspace} onOpenDetails={onOpenDetails} />
      </HostStoreProvider>
    </BindingProvider>,
  );
  return { onOpenDetails };
}

/** The page of the screen on screen: the one mounted last. */
function page() {
  const mounted = webViews[webViews.length - 1];
  if (mounted === undefined) throw new Error('no page was mounted');
  return mounted;
}

/** The page posts that it is ready, at the size it fitted. */
async function readyPage(cols: number = 80, rows: number = 24): Promise<void> {
  await act(async () => {
    page().post({ kind: 'ready', cols, rows });
  });
}

function viewOf(fake: FakeMassOrca): FakeTerminalView {
  const view = fake.terminals[0];
  if (view === undefined) throw new Error('the terminal was not attached');
  return view;
}

const textOf = (bytes: ArrayBuffer): string => new TextDecoder().decode(bytes);

describe('terminal screen', () => {
  it('opens the terminal at the size the page fits, and restores the screen from the snapshot', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptTerminal([snapshot]);
    await renderScreen(fake);

    expect(await screen.findByText('Loading terminal. Starting the shell on home-pc.')).toBeTruthy();
    await readyPage(100, 30);

    await waitFor(() => expect(fake.attaches).toEqual([{ terminal: 'term-1', cols: 100, rows: 30 }]));
    await waitFor(() =>
      expect(page().injected.some((script) => script.startsWith('window.massorcaTerminal.restore(80, 24, '))).toBe(true),
    );
    expect(screen.queryByText('Loading terminal. Starting the shell on home-pc.')).toBeNull();
  });

  it('says how the process exited, and takes no keystrokes after it', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptTerminal([snapshot, { tag: 'Exited', reason: 'exited', code: 1, signal: undefined }]);
    await renderScreen(fake);
    await readyPage();

    expect(await screen.findByText('Process exited with code 1.')).toBeTruthy();
    await waitFor(() => expect(page().injected).toContain('window.massorcaTerminal.setReadOnly(true);true;'));
  });

  it('keeps the screen while the host is unreachable, with the age of its last contact, and says when it is back', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptTerminal([snapshot]);
    await renderScreen(fake);
    await readyPage();
    await waitFor(() => expect(fake.terminals).toHaveLength(1));

    await act(async () => {
      viewOf(fake).push({
        tag: 'Link',
        link: { tag: 'Unverifiable', attempt: 1, retryInMs: 2000n, reason: 'connection closed' },
      });
    });

    expect(
      await screen.findByText(
        "home-pc can't be reached. Showing the last known state from a moment ago. Work that was running there may still be running.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText(/stopped/i)).toBeNull();
    expect(page().injected).toContain('window.massorcaTerminal.setReadOnly(true);true;');

    await act(async () => {
      viewOf(fake).push({ tag: 'Link', link: { tag: 'Live' } });
    });
    expect(await screen.findByText('Reconnected to home-pc.')).toBeTruthy();
  });

  it('shows why the attach was refused, and attaches again on Retry', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.failNext('attachTerminal', 'unreachable', 'home-pc is off');
    fake.scriptTerminal([snapshot]);
    await renderScreen(fake);
    await readyPage();

    expect(await screen.findByText('Could not open the terminal')).toBeTruthy();
    expect(screen.getByText('home-pc is off')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(fake.attaches).toEqual([{ terminal: 'term-1', cols: 80, rows: 24 }]));
    expect(screen.queryByText('Could not open the terminal')).toBeNull();
  });

  it('says the terminal could not start when its page fails to load, and loads it again on Retry', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    await renderScreen(fake);
    const failed = page();
    await act(async () => {
      failed.fail('net::ERR_FILE_NOT_FOUND');
    });

    expect(await screen.findByText('The terminal could not start')).toBeTruthy();
    expect(
      screen.getByText('The terminal view failed to load on this phone. The agent keeps running on home-pc.'),
    ).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));
    expect(page()).not.toBe(failed);
    expect(screen.queryByText('The terminal could not start')).toBeNull();
  });

  it('sends what the user types, with Ctrl applied to the next key only', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptTerminal([snapshot]);
    await renderScreen(fake);
    await readyPage();
    await waitFor(() => expect(fake.terminals).toHaveLength(1));

    await fireEvent.press(screen.getByRole('button', { name: 'Control, next key' }));
    await act(async () => {
      page().post({ kind: 'input', text: 'c' });
    });
    await act(async () => {
      page().post({ kind: 'input', text: 'l' });
    });

    await waitFor(() => expect(viewOf(fake).inputs.map(textOf)).toEqual(['\u0003', 'l']));
  });

  it('sends the arrow keys through the page, and the dock line with Enter', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptTerminal([snapshot]);
    await renderScreen(fake);
    await readyPage();
    await waitFor(() => expect(fake.terminals).toHaveLength(1));

    await fireEvent.press(screen.getByRole('button', { name: 'Up arrow' }));
    expect(page().injected).toContain('window.massorcaTerminal.key("up");true;');

    await fireEvent.changeText(screen.getByLabelText('Type a command'), 'ls');
    await fireEvent.press(screen.getByRole('button', { name: 'Send to terminal' }));
    await waitFor(() => expect(viewOf(fake).inputs.map(textOf)).toEqual(['ls\r']));
  });

  it('resizes the host when the page changes size, once for each new size', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptTerminal([snapshot]);
    await renderScreen(fake);
    await readyPage();
    await waitFor(() => expect(fake.terminals).toHaveLength(1));

    await act(async () => {
      page().post({ kind: 'resize', cols: 100, rows: 30 });
      page().post({ kind: 'resize', cols: 100, rows: 30 });
    });

    await waitFor(() => expect(viewOf(fake).resizes).toEqual([[100, 30]]));
  });

  it('shows the agent state of its workspace in the status pill', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.scriptAgentWatch([
      {
        tag: 'Item',
        item: {
          tag: 'Snapshot',
          sessions: [agentSession({ id: 's1', workspaceId: 'ws-1', activityState: 'waiting' })],
        },
      },
    ]);
    fake.scriptTerminal([snapshot]);
    await renderScreen(fake, 'ws-1');
    await readyPage();

    expect(await screen.findByText('Needs you')).toBeTruthy();
  });
});
