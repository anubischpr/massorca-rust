import { terminalSummary } from '../state/test-fixtures';
import { openDirectlyTarget, terminalRows } from './terminal-list-model';

describe('terminal rows', () => {
  it('names a terminal by its title, or by its shell when the program set no title', () => {
    const rows = terminalRows([
      terminalSummary({ id: 't-1', title: 'dev server', shell: 'pwsh' }),
      terminalSummary({ id: 't-2', title: undefined, shell: 'zsh', cwd: '/repos/ws-1/api' }),
      terminalSummary({ id: 't-3', title: '   ', shell: 'bash' }),
    ]);

    expect(rows.map((row) => row.name)).toEqual(['dev server', 'zsh', 'bash']);
    expect(rows[1]?.cwd).toBe('/repos/ws-1/api');
  });

  it('gives each state its word from the binding vocabulary, and "Unknown" to a state it does not know', () => {
    const rows = terminalRows([
      terminalSummary({ id: 't-1', state: 'live' }),
      terminalSummary({ id: 't-2', state: 'unverifiable' }),
      terminalSummary({ id: 't-3', state: 'exited' }),
      terminalSummary({ id: 't-4', state: 'stopped' }),
    ]);

    expect(rows.map((row) => row.word)).toEqual(['Live', 'Unverifiable', 'Exited', 'Unknown']);
    expect(rows.map((row) => row.state)).toEqual(['live', 'unverifiable', 'exited', 'unknown']);
  });
});

describe('the terminal opened at once', () => {
  it('is the only live terminal, even when the workspace has other terminals too', () => {
    expect(openDirectlyTarget([terminalSummary({ id: 't-1' }), terminalSummary({ id: 't-2', state: 'exited' })])).toBe('t-1');
  });

  it('is none when several terminals are live, when none is, or when there are none', () => {
    expect(openDirectlyTarget([terminalSummary({ id: 't-1' }), terminalSummary({ id: 't-2' })])).toBeUndefined();
    expect(openDirectlyTarget([terminalSummary({ id: 't-1', state: 'unverifiable' })])).toBeUndefined();
    expect(openDirectlyTarget([])).toBeUndefined();
  });
});
