import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import {
  readTerminalSources,
  renderTerminalPage,
  TERMINAL_ASSET_PATH,
  type TerminalAssetSources,
} from '../../scripts/generate-terminal-asset';

const mobileRoot = resolve(__dirname, '../..');
const sources = readTerminalSources(mobileRoot);

describe('the terminal page asset', () => {
  it('is the output of the generator for the committed template and xterm.js (decision 0080)', () => {
    const committed = readFileSync(resolve(mobileRoot, TERMINAL_ASSET_PATH), 'utf8').replace(/\r\n/g, '\n');
    expect(committed).toBe(renderTerminalPage(sources));
  });

  it('inlines every library, so the page loads nothing from the network', () => {
    const page = renderTerminalPage(sources);
    expect(page).not.toMatch(/<script[^>]*\ssrc=/i);
    expect(page).not.toMatch(/<link[^>]*stylesheet/i);
    expect(page).toContain(sources.xtermJs);
    expect(page).toContain(sources.fitAddonJs);
  });

  it('exposes the functions the app calls and posts the messages it reads', () => {
    const page = renderTerminalPage(sources);
    for (const name of ['write', 'restore', 'resize', 'key', 'setReadOnly', 'setTheme']) {
      expect(page).toContain(`${name}: function`);
    }
    for (const kind of ['ready', 'input', 'resize', 'error']) {
      expect(page).toContain(`kind: '${kind}'`);
    }
  });

  it('refuses a template without its markers, or a library that would close its own tag', () => {
    const broken: TerminalAssetSources = { ...sources, template: '<html></html>' };
    expect(() => renderTerminalPage(broken)).toThrow('the template needs exactly one __XTERM_CSS__');

    const closing: TerminalAssetSources = { ...sources, xtermJs: 'var s = "</script>";' };
    expect(() => renderTerminalPage(closing)).toThrow('__XTERM_JS__: the library closes its own tag');
  });
});
