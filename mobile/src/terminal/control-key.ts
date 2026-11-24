/**
 * What a typed text becomes while Ctrl is armed: a letter, or one of `@[\]^_`, becomes its control code; space and `?`
 * become NUL and DEL. Anything else is typed as it is.
 */
export function applyControl(text: string): string {
  if (text.length !== 1) return text;
  if (text === ' ') return '\u0000';
  if (text === '?') return '\u007f';
  const code = text.toUpperCase().charCodeAt(0);
  return code >= 0x40 && code <= 0x5f ? String.fromCharCode(code % 0x20) : text;
}
