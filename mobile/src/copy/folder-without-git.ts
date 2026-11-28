// Copy of the sheet a folder without git opens from its No Git badge (M-WS-05; docs/design/screens/android.md, 6.5).

export const NO_GIT_TITLE = 'This folder is not a git repository';
export const NO_GIT_BODY = (folder: string): string =>
  `${folder} is a plain folder, so there are no branches or diffs to show. Initialize Git to track changes and use worktrees.`;
export const NO_GIT_GATE = 'Needs MassOrca 0.9.4 or newer on This PC.';
export const INITIALIZE_GIT = 'Initialize Git';
export const NOT_NOW = 'Not now';
/** The TalkBack custom action that opens the same sheet as the badge. */
export const NO_GIT_ACTION = 'About this folder';
