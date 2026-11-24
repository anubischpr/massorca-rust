import { isValidHostName, suggestHostName } from './host-name';

describe('isValidHostName', () => {
  it('accepts names of the host store rule', () => {
    for (const name of ['vps', 'home-pc', 'a_b', 'a', 'a'.repeat(32)]) {
      expect(isValidHostName(name)).toBe(true);
    }
  });

  it('rejects names the host store refuses', () => {
    for (const name of ['', '-vps', 'vps.example', 'VPS', 'a'.repeat(33), 'local']) {
      expect(isValidHostName(name)).toBe(false);
    }
  });
});

describe('suggestHostName', () => {
  it('takes the first label of the desktop name', () => {
    expect(suggestHostName('vps.example.ts.net')).toBe('vps');
  });

  it('cleans spaces and case into a valid name', () => {
    expect(suggestHostName('  My Desktop  ')).toBe('my-desktop');
    expect(suggestHostName('This PC')).toBe('this-pc');
  });

  it('returns nothing usable when no valid name is left', () => {
    expect(suggestHostName('...')).toBe('');
    expect(suggestHostName('local')).toBe('');
  });

  it('keeps at most 32 characters', () => {
    expect(suggestHostName('x'.repeat(40))).toHaveLength(32);
  });
});
