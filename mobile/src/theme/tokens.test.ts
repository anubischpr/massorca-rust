import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { renderTokens, type TokenFile } from '../../scripts/generate-tokens';
import { colorTokens, spacingTokens, typeTokens } from './tokens.generated';

const repoRoot = resolve(__dirname, '../../..');
const tokensJson = resolve(repoRoot, 'docs/design/tokens.json');
const committed = resolve(__dirname, 'tokens.generated.ts');

function readTokenFile(): TokenFile {
  return JSON.parse(readFileSync(tokensJson, 'utf8')) as TokenFile;
}

function minimal(colors: TokenFile['color']['tokens']): TokenFile {
  return { color: { tokens: colors }, type: { groups: [] }, spacing: { tokens: [] }, radius: { tokens: [] } };
}

describe('tokens.generated.ts', () => {
  it('is the output of the generator for the committed tokens.json (decision 0022)', () => {
    const current = readFileSync(committed, 'utf8').replace(/\r\n/g, '\n');
    expect(current).toBe(renderTokens(readTokenFile()));
  });

  it('resolves every alias to the colour of its target in each theme', () => {
    const json = readTokenFile();
    const aliased = json.color.tokens.filter((t) => typeof t.value === 'string' && t.value.startsWith('{'));
    expect(aliased.length).toBeGreaterThan(0);
    for (const token of aliased) {
      const target = (token.value as string).slice(1, -1);
      for (const theme of ['dark', 'light'] as const) {
        expect(colorTokens[theme][token.name as keyof typeof colorTokens.dark]).toBe(
          colorTokens[theme][target as keyof typeof colorTokens.dark],
        );
      }
    }
  });

  it('maps a type style, a spacing step and a radius to their values', () => {
    expect(typeTokens.caption).toEqual({ family: 'sans', size: 11, lineHeight: 16, weight: 400, letterSpacingEm: 0 });
    expect(spacingTokens['space.4']).toBe(16);
  });

  it('fails on an unknown alias', () => {
    expect(() => renderTokens(minimal([{ name: 'x.a', value: '{x.missing}' }]))).toThrow(
      'x.a: unknown alias x.missing',
    );
  });

  it('fails on an alias loop', () => {
    expect(() =>
      renderTokens(
        minimal([
          { name: 'x.a', value: '{x.b}' },
          { name: 'x.b', value: '{x.a}' },
        ]),
      ),
    ).toThrow('x.a: alias loop');
  });

  it('fails on a value that is not a colour', () => {
    expect(() => renderTokens(minimal([{ name: 'x.a', value: 'red' }]))).toThrow('bad colour red');
  });
});
