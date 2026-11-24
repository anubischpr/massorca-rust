import type { GitFileStatus, GitStatusSummary } from '../binding/types';
import { aheadBehindLine, changeKindOf, changeRows, conflictCount } from './changes-model';

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

function status(overrides: Partial<GitStatusSummary>): GitStatusSummary {
  return {
    branch: 'main',
    ahead: undefined,
    behind: undefined,
    operation: undefined,
    files: [],
    truncated: false,
    ...overrides,
  };
}

describe('changeKindOf', () => {
  it('shows a conflict before any other change', () => {
    expect(changeKindOf(file({ path: 'a.ts', conflict: 'both_modified', unstaged: 'modified' }))).toBe('conflict');
  });

  it('shows the working tree change over the staged one, and the staged one alone', () => {
    expect(changeKindOf(file({ path: 'a.ts', staged: 'added', unstaged: 'modified' }))).toBe('modified');
    expect(changeKindOf(file({ path: 'a.ts', staged: 'deleted' }))).toBe('deleted');
  });

  it('reads the binding words of each kind, and an unknown one as changed', () => {
    expect(changeKindOf(file({ path: 'a.ts', unstaged: 'renamed' }))).toBe('renamed');
    expect(changeKindOf(file({ path: 'a.ts', unstaged: 'type_changed' }))).toBe('typeChanged');
    expect(changeKindOf(file({ path: 'a.ts', unstaged: 'something_new' }))).toBe('unknown');
  });

  it('shows a file git does not track as untracked', () => {
    expect(changeKindOf(file({ path: 'notes.txt', untracked: true }))).toBe('untracked');
  });
});

describe('changeRows', () => {
  it('splits each path into its name and its folder, and leaves a top-level file without one', () => {
    expect(changeRows([file({ path: 'src/app/main.ts', unstaged: 'modified' }), file({ path: 'README.md' })])).toEqual([
      { path: 'src/app/main.ts', kind: 'modified', name: 'main.ts', directory: 'src/app' },
      { path: 'README.md', kind: 'unknown', name: 'README.md', directory: undefined },
    ]);
  });
});

describe('conflictCount', () => {
  it('counts the files with a conflict', () => {
    expect(
      conflictCount([file({ path: 'a', conflict: 'both_added' }), file({ path: 'b' }), file({ path: 'c', conflict: 'x' })]),
    ).toBe(2);
  });
});

describe('aheadBehindLine', () => {
  it('reads the counts of a branch with an upstream, and nothing without one', () => {
    expect(aheadBehindLine(status({ ahead: 2n, behind: 0n }))).toBe('2 ahead, 0 behind');
    expect(aheadBehindLine(status({ ahead: 1n }))).toBe('1 ahead, 0 behind');
    expect(aheadBehindLine(status({}))).toBeUndefined();
  });
});
