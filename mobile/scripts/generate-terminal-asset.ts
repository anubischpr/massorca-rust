// Builds the terminal page the WebView loads from the APK (decision 0080, task A10): mobile/assets/terminal/page.html with
// xterm.js, its CSS and the fit addon inlined, so the terminal works offline. `node scripts/generate-terminal-asset.ts`
// rewrites the asset; `--check` fails when the committed asset differs, as `npm run tokens:check` does for the tokens.

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

export const TERMINAL_ASSET_PATH = 'android/app/src/main/assets/terminal/index.html';

export interface TerminalAssetSources {
  readonly template: string;
  readonly xtermCss: string;
  readonly xtermJs: string;
  readonly fitAddonJs: string;
}

const MARKERS = ['__XTERM_CSS__', '__XTERM_JS__', '__FIT_ADDON_JS__'] as const;

/** The page with its libraries inlined. Each marker must appear once, and no library may close its own tag. */
export function renderTerminalPage(sources: TerminalAssetSources): string {
  const libraries = [sources.xtermCss, sources.xtermJs, sources.fitAddonJs];
  let page = sources.template;
  MARKERS.forEach((marker, index) => {
    if (page.split(marker).length !== 2) throw new Error(`the template needs exactly one ${marker}`);
    const library = libraries[index] ?? '';
    if (/<\/(script|style)/i.test(library)) throw new Error(`${marker}: the library closes its own tag`);
    // A function replacement keeps `$&` and the like in the library from being read as patterns.
    page = page.replace(marker, () => library);
  });
  return page;
}

/** The sources the generator reads, under the mobile/ directory `root`. */
export function readTerminalSources(root: string): TerminalAssetSources {
  const read = (path: string): string => readFileSync(resolve(root, path), 'utf8');
  return {
    template: read('assets/terminal/page.html'),
    xtermCss: read('node_modules/@xterm/xterm/css/xterm.css'),
    xtermJs: read('node_modules/@xterm/xterm/lib/xterm.js'),
    fitAddonJs: read('node_modules/@xterm/addon-fit/lib/addon-fit.js'),
  };
}

function main(): void {
  const root = resolve(dirname(process.argv[1] ?? '.'), '..');
  const generated = renderTerminalPage(readTerminalSources(root));
  const outPath = resolve(root, TERMINAL_ASSET_PATH);
  if (process.argv.includes('--check')) {
    const committed = readFileSync(outPath, 'utf8').replace(/\r\n/g, '\n');
    if (committed !== generated) {
      process.stderr.write(`${TERMINAL_ASSET_PATH} is out of date; run \`node scripts/generate-terminal-asset.ts\`\n`);
      process.exitCode = 1;
    }
    return;
  }
  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, generated);
}

if (process.argv[1]?.endsWith('generate-terminal-asset.ts')) {
  main();
}
