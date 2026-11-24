// Generates mobile/src/theme/tokens.generated.ts from docs/design/tokens.json (decision 0022, same contract as
// crates/ui/tests/tokens_are_generated.rs). `npm run tokens` rewrites the file; `npm run tokens:check` fails if it differs.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

export interface ColorToken {
  name: string;
  value: string | Partial<Record<'dark' | 'light', string>>;
}

export interface ScalarToken {
  name: string;
  value: string;
}

export interface TypeStyle {
  name: string;
  family?: string;
  fontSize: string;
  lineHeight: string;
  fontWeight: number;
  letterSpacing?: string;
}

export interface TypeGroup {
  name: string;
  family: string;
  styles: TypeStyle[];
}

/** The part of docs/design/tokens.json this generator reads. */
export interface TokenFile {
  color: { tokens: ColorToken[] };
  type: { groups: TypeGroup[] };
  spacing: { tokens: ScalarToken[] };
  radius: { tokens: ScalarToken[] };
}

type Theme = 'dark' | 'light';

const THEMES: readonly Theme[] = ['dark', 'light'];
const MAX_ALIAS_DEPTH = 8;
const COLOR_LITERAL = /^(#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?|rgba\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*,\s*[\d.]+\s*\))$/;
const ALIAS = /^\{([^{}]+)\}$/;

function assertUnique(names: readonly string[], what: string): void {
  const seen = new Set<string>();
  for (const name of names) {
    if (seen.has(name)) throw new Error(`duplicate ${what} ${name}`);
    seen.add(name);
  }
}

/** Resolves `{alias}` chains for one theme; fails on an unknown alias or a loop. */
function resolveColor(name: string, theme: Theme, raw: ReadonlyMap<string, ColorToken['value']>): string {
  let value = raw.get(name);
  for (let depth = 0; depth < MAX_ALIAS_DEPTH; depth++) {
    if (value === undefined) throw new Error(`${name}: unknown alias`);
    const text = (typeof value === 'string' ? value : value[theme])?.trim();
    if (text === undefined) throw new Error(`${name}: no ${theme} value`);
    const alias = ALIAS.exec(text);
    if (alias === null) {
      if (!COLOR_LITERAL.test(text)) throw new Error(`${name}: bad colour ${text}`);
      return text;
    }
    const target = alias[1] ?? '';
    if (!raw.has(target)) throw new Error(`${name}: unknown alias ${target}`);
    value = raw.get(target);
  }
  throw new Error(`${name}: alias loop`);
}

/** Pixel value of a length; Android styles are written in sp, which is the same number in the app. */
function pixels(name: string, value: string): number {
  const match = /^(-?\d+(?:\.\d+)?)(px|sp)$/.exec(value);
  if (match === null) throw new Error(`${name}: not a length in px or sp: ${value}`);
  return Number(match[1]);
}

/** A TypeScript property key: bare when it is an identifier, quoted otherwise. */
function key(name: string): string {
  return /^[A-Za-z_$][\w$]*$/.test(name) ? name : `'${name}'`;
}

function colorBlocks(json: TokenFile): string {
  const tokens = json.color.tokens;
  assertUnique(
    tokens.map((t) => t.name),
    'colour token',
  );
  const raw = new Map(tokens.map((t) => [t.name, t.value] as const));
  const themes = THEMES.map((theme) => {
    const lines = tokens.map((t) => `    ${key(t.name)}: '${resolveColor(t.name, theme, raw)}',`);
    return [`  ${theme}: {`, ...lines, '  },'].join('\n');
  });
  return ['export const colorTokens = {', ...themes, '} as const;'].join('\n');
}

function typeBlock(json: TokenFile): string {
  const styles = json.type.groups.flatMap((group) =>
    group.styles.map((style) => ({
      style,
      family: style.family ?? (style.name.includes('mono') ? 'mono' : group.family),
    })),
  );
  assertUnique(
    styles.map((s) => s.style.name),
    'type style',
  );
  const lines = styles.map(({ style, family }) => {
    if (family !== 'sans' && family !== 'mono') throw new Error(`${style.name}: unknown family ${family}`);
    const letterSpacingEm = style.letterSpacing === undefined ? 0 : Number(style.letterSpacing.replace(/em$/, ''));
    return [
      `  ${key(style.name)}: {`,
      `    family: '${family}',`,
      `    size: ${pixels(style.name, style.fontSize)},`,
      `    lineHeight: ${pixels(style.name, style.lineHeight)},`,
      `    weight: ${style.fontWeight},`,
      `    letterSpacingEm: ${letterSpacingEm},`,
      '  },',
    ].join('\n');
  });
  return ['export const typeTokens = {', ...lines, '} as const;'].join('\n');
}

function scalarBlock(name: string, tokens: readonly ScalarToken[]): string {
  assertUnique(
    tokens.map((t) => t.name),
    `${name} token`,
  );
  const lines = tokens.map((t) => `  ${key(t.name)}: ${pixels(t.name, t.value)},`);
  return [`export const ${name} = {`, ...lines, '} as const;'].join('\n');
}

const HEADER = [
  '// @generated from docs/design/tokens.json by mobile/scripts/generate-tokens.ts. Do not edit by hand (decision 0022).',
  '',
  "export type FontFamily = 'sans' | 'mono';",
  '',
  'export interface TextStyleToken {',
  '  readonly family: FontFamily;',
  '  readonly size: number;',
  '  readonly lineHeight: number;',
  '  readonly weight: number;',
  '  readonly letterSpacingEm: number;',
  '}',
].join('\n');

/** The full text of tokens.generated.ts for a parsed tokens.json. */
export function renderTokens(json: TokenFile): string {
  const sections = [
    HEADER,
    colorBlocks(json),
    "export type ColorTheme = keyof typeof colorTokens;",
    "export type ColorTokenName = keyof (typeof colorTokens)['dark'];",
    typeBlock(json),
    scalarBlock('spacingTokens', json.spacing.tokens),
    scalarBlock('radiusTokens', json.radius.tokens),
  ];
  return `${sections.join('\n\n')}\n`;
}

function main(): void {
  const mobileRoot = resolve(dirname(process.argv[1] ?? '.'), '..');
  const tokensPath = resolve(mobileRoot, '../docs/design/tokens.json');
  const outPath = resolve(mobileRoot, 'src/theme/tokens.generated.ts');
  const generated = renderTokens(JSON.parse(readFileSync(tokensPath, 'utf8')) as TokenFile);
  if (process.argv.includes('--check')) {
    const committed = readFileSync(outPath, 'utf8').replace(/\r\n/g, '\n');
    if (committed !== generated) {
      process.stderr.write('src/theme/tokens.generated.ts is out of date; run `npm run tokens`\n');
      process.exitCode = 1;
    }
    return;
  }
  writeFileSync(outPath, generated);
}

if (process.argv[1]?.endsWith('generate-tokens.ts')) {
  main();
}
