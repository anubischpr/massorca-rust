// The terminal page (mobile/assets/terminal/page.html, built into the Android assets) and the app talk through the
// WebView: the page posts JSON messages, and the app runs one of the page's functions with injectJavaScript.

import { toBase64 } from '../binding/base64';
import type { Palette } from '../theme/palette';

/** Where the WebView loads the page: the Android asset that scripts/generate-terminal-asset.ts builds. */
export const TERMINAL_PAGE_URI = 'file:///android_asset/terminal/index.html';

/** The keys the page sends itself, so that the arrows follow the program's cursor mode. */
export type PageKey = 'esc' | 'tab' | 'up' | 'down' | 'left' | 'right';

/** The page's colours, from the terminal tokens of the theme in use. */
export interface TerminalTheme {
  readonly background: string;
  readonly foreground: string;
  readonly cursor: string;
  readonly selectionBackground: string;
  readonly black: string;
  readonly red: string;
  readonly green: string;
  readonly yellow: string;
  readonly blue: string;
  readonly magenta: string;
  readonly cyan: string;
  readonly white: string;
}

/** What the page posts: the size it fitted, what the user typed, a size change, or a failure of its own. */
export type PageMessage =
  | { readonly kind: 'ready'; readonly cols: number; readonly rows: number }
  | { readonly kind: 'input'; readonly text: string }
  | { readonly kind: 'resize'; readonly cols: number; readonly rows: number }
  | { readonly kind: 'error'; readonly message: string };

/** Reads one message the page posted. Anything that is not one of them is ignored, as undefined. */
export function parsePageMessage(raw: string): PageMessage | undefined {
  const value = parseRecord(raw);
  if (value === undefined) return undefined;
  const { kind } = value;
  if (kind === 'ready' || kind === 'resize') {
    const { cols, rows } = value;
    if (!isSize(cols) || !isSize(rows)) return undefined;
    return kind === 'ready' ? { kind: 'ready', cols, rows } : { kind: 'resize', cols, rows };
  }
  if (kind === 'input' && typeof value.text === 'string') return { kind: 'input', text: value.text };
  if (kind === 'error' && typeof value.message === 'string') return { kind: 'error', message: value.message };
  return undefined;
}

export function terminalTheme(palette: Palette): TerminalTheme {
  return {
    background: palette['terminal.bg'],
    foreground: palette['terminal.fg'],
    cursor: palette['terminal.cursor'],
    selectionBackground: palette['terminal.selection'],
    black: palette['terminal.ansi.black'],
    red: palette['terminal.ansi.red'],
    green: palette['terminal.ansi.green'],
    yellow: palette['terminal.ansi.yellow'],
    blue: palette['terminal.ansi.blue'],
    magenta: palette['terminal.ansi.magenta'],
    cyan: palette['terminal.ansi.cyan'],
    white: palette['terminal.ansi.white'],
  };
}

/** The script that shows output bytes in the page. */
export function writeScript(bytes: ArrayBuffer): string {
  return callPage('write', [toBase64(bytes)]);
}

/** The script that replaces the page's screen with a snapshot at the size the host holds it. */
export function restoreScript(cols: number, rows: number, bytes: ArrayBuffer): string {
  return callPage('restore', [cols, rows, toBase64(bytes)]);
}

export function resizeScript(cols: number, rows: number): string {
  return callPage('resize', [cols, rows]);
}

/** The script that makes the page send a key, as its program expects it. */
export function keyScript(key: PageKey): string {
  return callPage('key', [key]);
}

/** While the host is out of contact the page takes no keystrokes. */
export function readOnlyScript(readOnly: boolean): string {
  return callPage('setReadOnly', [readOnly]);
}

export function themeScript(theme: TerminalTheme): string {
  return callPage('setTheme', [theme]);
}

type PageArgument = string | number | boolean | TerminalTheme;

/** A call to one of the page's functions, each argument as JSON, then `true` as injectJavaScript expects. */
function callPage(name: string, args: readonly PageArgument[]): string {
  return `window.massorcaTerminal.${name}(${args.map((arg) => JSON.stringify(arg)).join(', ')});true;`;
}

function parseRecord(raw: string): Record<string, unknown> | undefined {
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return undefined;
  }
  return isRecord(value) ? value : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isSize(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value > 0;
}
