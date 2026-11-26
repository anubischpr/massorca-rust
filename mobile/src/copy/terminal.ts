// Terminal screen copy (screen spec 9.1). Host names are the names the phone gave the host.

export const TERMINAL_TITLE = 'Terminal';
export const LOADING_TERMINAL = (host: string): string => `Loading terminal. Starting the shell on ${host}.`;
export const ENGINE_FAILED_TITLE = 'The terminal could not start';
export const ENGINE_FAILED_LINE = (host: string): string =>
  `The terminal view failed to load on this phone. The agent keeps running on ${host}.`;
export const ATTACH_FAILED_TITLE = 'Could not open the terminal';
export const TYPE_COMMAND = 'Type a command';
export const SEND_TO_TERMINAL = 'Send to terminal';

/** The Terminal view of a workspace: its list of terminals, and the word each terminal state shows. */
export const LOADING_TERMINALS = 'Loading terminals';
export const TERMINALS_FAILED_TITLE = 'Could not load the terminals';
export const NO_TERMINAL_TITLE = 'No terminal in this workspace';
export const NO_TERMINAL_LINE = 'Terminals in this workspace are listed here.';
export const TERMINALS_OVERLINE = (count: number): string => `Terminals (${count})`;
/** The state word of each terminal state, in the binding's vocabulary. Loss of contact is never "stopped". */
export const TERMINAL_STATE_WORDS = {
  live: 'Live',
  unverifiable: 'Unverifiable',
  exited: 'Exited',
  unknown: 'Unknown',
} as const;

/** The process ended: its code, or its signal, as the spec words it. */
export function exitedLine(code: number | undefined, signal: string | undefined): string {
  if (code !== undefined) return `Process exited with code ${code}.`;
  if (signal !== undefined) return `Process ended by signal ${signal}.`;
  return 'Process exited.';
}

/** The key row (M-TM-02): the visible label and the name a screen reader reads. */
export const KEY_ROW = [
  { key: 'esc', label: 'Esc', name: 'Escape' },
  { key: 'tab', label: 'Tab', name: 'Tab' },
  { key: 'ctrl', label: 'Ctrl', name: 'Control, next key' },
  { key: 'up', label: '↑', name: 'Up arrow' },
  { key: 'down', label: '↓', name: 'Down arrow' },
  { key: 'left', label: '←', name: 'Left arrow' },
  { key: 'right', label: '→', name: 'Right arrow' },
] as const;
