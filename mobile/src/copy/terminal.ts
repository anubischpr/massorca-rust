// Terminal screen copy (screen spec 9.1). Host names are the names the phone gave the host.

export const TERMINAL_TITLE = 'Terminal';
export const STARTING_SHELL = 'Starting shell';
export const LOADING_TERMINAL = (host: string): string => `Loading terminal. Starting the shell on ${host}.`;
export const ENGINE_FAILED_TITLE = 'The terminal could not start';
export const ENGINE_FAILED_LINE = (host: string): string =>
  `The terminal view failed to load on this phone. The agent keeps running on ${host}.`;
export const ATTACH_FAILED_TITLE = 'Could not open the terminal';
export const READ_ONLY_LINE = 'Input is paused until the host is reachable again.';
export const TYPE_COMMAND = 'Type a command';
export const SEND_TO_TERMINAL = 'Send to terminal';

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
