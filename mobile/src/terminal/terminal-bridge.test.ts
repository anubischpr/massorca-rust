import { fromBase64 } from '../binding/base64';
import { colorTokens } from '../theme/tokens.generated';
import {
  keyScript,
  parsePageMessage,
  readOnlyScript,
  resizeScript,
  restoreScript,
  terminalTheme,
  themeScript,
  writeScript,
} from './terminal-bridge';

function bytesOf(text: string): ArrayBuffer {
  return new TextEncoder().encode(text).buffer;
}

/** The base64 payload of a script whose first argument is a string, read back to text. */
function payloadText(script: string): string {
  const match = /"([A-Za-z0-9+/=]*)"/.exec(script);
  if (match?.[1] === undefined) throw new Error(`no payload in ${script}`);
  return new TextDecoder().decode(fromBase64(match[1]));
}

describe('parsePageMessage', () => {
  it('reads the size the page fitted, a size change, what was typed, and its own failure', () => {
    expect(parsePageMessage('{"kind":"ready","cols":80,"rows":24}')).toEqual({ kind: 'ready', cols: 80, rows: 24 });
    expect(parsePageMessage('{"kind":"resize","cols":100,"rows":30}')).toEqual({ kind: 'resize', cols: 100, rows: 30 });
    expect(parsePageMessage('{"kind":"input","text":"ls\\r"}')).toEqual({ kind: 'input', text: 'ls\r' });
    expect(parsePageMessage('{"kind":"error","message":"xterm.js did not load"}')).toEqual({
      kind: 'error',
      message: 'xterm.js did not load',
    });
  });

  it('ignores anything that is not one of its messages', () => {
    expect(parsePageMessage('not json')).toBeUndefined();
    expect(parsePageMessage('[1,2]')).toBeUndefined();
    expect(parsePageMessage('{"kind":"clipboard","text":"x"}')).toBeUndefined();
    expect(parsePageMessage('{"kind":"input"}')).toBeUndefined();
  });

  it('refuses a size that is not a positive whole number', () => {
    expect(parsePageMessage('{"kind":"ready","cols":0,"rows":24}')).toBeUndefined();
    expect(parsePageMessage('{"kind":"resize","cols":80.5,"rows":24}')).toBeUndefined();
    expect(parsePageMessage('{"kind":"ready","cols":"80","rows":24}')).toBeUndefined();
    expect(parsePageMessage('{"kind":"resize","cols":80}')).toBeUndefined();
  });
});

describe('page scripts', () => {
  it('hands the page the bytes it writes, as base64 it decodes', () => {
    const script = writeScript(bytesOf('ls\r\n'));

    expect(script.startsWith('window.massorcaTerminal.write("')).toBe(true);
    expect(script.endsWith('");true;')).toBe(true);
    expect(payloadText(script)).toBe('ls\r\n');
  });

  it('restores a snapshot at the size the host holds it', () => {
    const script = restoreScript(100, 30, bytesOf('$ '));

    expect(script.startsWith('window.massorcaTerminal.restore(100, 30, "')).toBe(true);
    expect(payloadText(script)).toBe('$ ');
  });

  it('sends the size, the key, the read-only flag and the theme as JSON arguments', () => {
    expect(resizeScript(100, 30)).toBe('window.massorcaTerminal.resize(100, 30);true;');
    expect(keyScript('up')).toBe('window.massorcaTerminal.key("up");true;');
    expect(readOnlyScript(true)).toBe('window.massorcaTerminal.setReadOnly(true);true;');
    expect(themeScript(terminalTheme(colorTokens.dark))).toContain(
      `"background":"${colorTokens.dark['terminal.bg']}"`,
    );
  });

  it('takes the page colours from the terminal tokens', () => {
    const theme = terminalTheme(colorTokens.light);

    expect(theme.background).toBe(colorTokens.light['terminal.bg']);
    expect(theme.foreground).toBe(colorTokens.light['terminal.fg']);
    expect(theme.magenta).toBe(colorTokens.light['terminal.ansi.magenta']);
  });
});
