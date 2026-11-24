// Source control copy, the Changes segment (screen spec 10.3, read only in M5).

export const SOURCE_CONTROL_TITLE = 'Source control';
export const LOADING_CHANGES = 'Loading changes';
export const CHANGES_FAILED_TITLE = 'Could not read the changes';
export const NO_GIT_TITLE = 'Git is not available in this folder';
export const NO_CHANGES_TITLE = 'No changes';
export const NO_CHANGES_LINE = 'Everything is committed.';
export const DETACHED_HEAD = 'Detached HEAD';
export const TRUNCATED_LINE = 'Only the first files are shown.';
export const CHANGES_OVERLINE = (count: number): string => `Changes (${count})`;
export const CONFLICTS_LINE = (count: number, host: string): string =>
  `${count} ${count === 1 ? 'file has' : 'files have'} conflicts. Resolve them on ${host}.`;
export const AHEAD_BEHIND = (ahead: bigint, behind: bigint): string => `${ahead} ahead, ${behind} behind`;

/** The word and the letter of each kind of change; the word is what a screen reader says. */
export const CHANGE_KIND_WORDS = {
  conflict: { letter: '!', word: 'Conflict' },
  added: { letter: 'A', word: 'Added' },
  untracked: { letter: 'U', word: 'Untracked' },
  modified: { letter: 'M', word: 'Modified' },
  deleted: { letter: 'D', word: 'Deleted' },
  renamed: { letter: 'R', word: 'Renamed' },
  copied: { letter: 'C', word: 'Copied' },
  typeChanged: { letter: 'T', word: 'Type changed' },
  unknown: { letter: '?', word: 'Changed' },
} as const;
