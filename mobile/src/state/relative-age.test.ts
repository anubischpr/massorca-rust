import { formatAge } from './relative-age';

describe('formatAge', () => {
  it('says a moment ago under one minute', () => {
    expect(formatAge(0)).toBe('a moment ago');
    expect(formatAge(59_999)).toBe('a moment ago');
  });

  it('counts minutes, hours and days', () => {
    expect(formatAge(5 * 60_000)).toBe('5 min ago');
    expect(formatAge(3 * 3_600_000)).toBe('3 h ago');
    expect(formatAge(2 * 86_400_000)).toBe('2 d ago');
  });

  it('treats a time in the future as now', () => {
    expect(formatAge(-5_000)).toBe('a moment ago');
  });
});
