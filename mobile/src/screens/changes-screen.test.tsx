import { fireEvent, render, screen } from '@testing-library/react-native';

import { BindingProvider } from '../binding/binding-context';
import { FakeMassOrca } from '../binding/fake';
import type { GitFileStatus, GitStatusSummary } from '../binding/types';
import { LOADING_CHANGES } from '../copy/changes';
import { HostStoreProvider } from '../state/host-store-context';
import { hostSummary, StalledGitStatusFake } from '../state/test-fixtures';
import { ChangesScreen } from './changes-screen';

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

function file(overrides: Partial<GitFileStatus> & { readonly path: string }): GitFileStatus {
  return {
    originalPath: undefined,
    staged: undefined,
    unstaged: undefined,
    untracked: false,
    conflict: undefined,
    ...overrides,
  };
}

function gitStatus(overrides: Partial<GitStatusSummary>): GitStatusSummary {
  return { branch: 'feat/billing', ahead: undefined, behind: undefined, operation: undefined, files: [], truncated: false, ...overrides };
}

async function renderChanges(fake: FakeMassOrca, noGit: boolean = false) {
  const onOpenDetails = jest.fn();
  await render(
    <BindingProvider binding={fake}>
      <HostStoreProvider binding={fake}>
        <ChangesScreen host="home-pc" workspace="ws-1" noGit={noGit} onOpenDetails={onOpenDetails} />
      </HostStoreProvider>
    </BindingProvider>,
  );
  return { onOpenDetails };
}

describe('changes of a workspace', () => {
  it('shows the branch with its counts, and each changed file with its kind and folder', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.setGitStatus(
      gitStatus({
        ahead: 2n,
        behind: 0n,
        files: [file({ path: 'src/app.ts', unstaged: 'modified' }), file({ path: 'README.md', staged: 'added' })],
      }),
    );
    await renderChanges(fake);

    expect(await screen.findByText('feat/billing')).toBeTruthy();
    expect(screen.getByText('2 ahead, 0 behind')).toBeTruthy();
    expect(screen.getByText('Changes (2)')).toBeTruthy();
    expect(screen.getByLabelText('Modified, src/app.ts')).toBeTruthy();
    expect(screen.getByText('src')).toBeTruthy();
    expect(screen.getByLabelText('Added, README.md')).toBeTruthy();
  });

  it('says there are no changes when the tree is clean', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.setGitStatus(gitStatus({ files: [] }));
    await renderChanges(fake);

    expect(await screen.findByText('No changes')).toBeTruthy();
    expect(screen.getByText('Everything is committed.')).toBeTruthy();
  });

  it('shows that it is loading until the status arrives', async () => {
    await renderChanges(new StalledGitStatusFake({ hosts: [hostSummary()] }));

    expect(await screen.findByLabelText(LOADING_CHANGES)).toBeTruthy();
  });

  it('shows why the status cannot be read, and reads it again on Retry', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.failNext('gitStatus', 'unreachable', 'home-pc is off');
    fake.setGitStatus(gitStatus({ files: [file({ path: 'a.ts', untracked: true })] }));
    await renderChanges(fake);

    expect(await screen.findByText('Could not read the changes')).toBeTruthy();
    expect(screen.getByText('home-pc is off')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByText('Changes (1)')).toBeTruthy();
  });

  it('keeps the last list, with the age of the last contact, while the link is lost', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.setGitStatus(gitStatus({ files: [file({ path: 'a.ts', untracked: true })] }));
    fake.scriptGitWatch([
      { tag: 'Item', item: { ready: true, kinds: ['worktree'], degraded: false, reason: undefined } },
      { tag: 'Link', link: { tag: 'Unverifiable', attempt: 1, retryInMs: 2000n, reason: 'connection closed' } },
    ]);
    await renderChanges(fake);

    expect(
      await screen.findByText(
        "home-pc can't be reached. Showing the last known state from a moment ago. Work that was running there may still be running.",
      ),
    ).toBeTruthy();
    expect(screen.getByText('Changes (1)')).toBeTruthy();
    expect(screen.queryByText(/stopped/i)).toBeNull();
  });

  it('says the conflicts and marks each conflicted file', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.setGitStatus(gitStatus({ files: [file({ path: 'src/a.ts', conflict: 'both_modified' })] }));
    await renderChanges(fake);

    expect(await screen.findByText('1 file has conflicts. Resolve them on home-pc.')).toBeTruthy();
    expect(screen.getByLabelText('Conflict, src/a.ts')).toBeTruthy();
  });

  it('names a detached head instead of a branch', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.setGitStatus(gitStatus({ branch: undefined, files: [] }));
    await renderChanges(fake);

    expect(await screen.findByText('Detached HEAD')).toBeTruthy();
  });

  it('says git is not available in a plain folder, and reads no status for it', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    await renderChanges(fake, true);

    expect(await screen.findByText('Git is not available in this folder')).toBeTruthy();
    expect(fake.calls).not.toContain('gitStatus');
  });
});
