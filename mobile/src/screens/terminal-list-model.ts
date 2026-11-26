import type { TerminalSummary } from '../binding/types';
import { TERMINAL_STATE_WORDS } from '../copy/terminal';

/** The states of a terminal in the binding's vocabulary; any other value from the host is `unknown`. */
export type TerminalState = keyof typeof TERMINAL_STATE_WORDS;

/** One row of the Terminal view: the name it shows, its folder, and its state word. */
export interface TerminalRow {
  readonly id: string;
  /** The title the program set, or its shell when it set none. */
  readonly name: string;
  readonly cwd: string;
  readonly state: TerminalState;
  readonly word: string;
}

export function terminalState(state: string): TerminalState {
  return state === 'live' || state === 'unverifiable' || state === 'exited' ? state : 'unknown';
}

export function terminalRows(terminals: readonly TerminalSummary[]): TerminalRow[] {
  return terminals.map((terminal) => {
    const state = terminalState(terminal.state);
    return {
      id: terminal.id,
      name: terminal.title === undefined || terminal.title.trim() === '' ? terminal.shell : terminal.title,
      cwd: terminal.cwd,
      state,
      word: TERMINAL_STATE_WORDS[state],
    };
  });
}

/** The terminal the view opens at once: the only live one. Undefined when there are none live, or several. */
export function openDirectlyTarget(terminals: readonly TerminalSummary[]): string | undefined {
  const live = terminals.filter((terminal) => terminalState(terminal.state) === 'live');
  return live.length === 1 ? live[0]?.id : undefined;
}
