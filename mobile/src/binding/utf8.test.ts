import { encodeUtf8 } from './utf8';

function bytesOf(buffer: ArrayBuffer): number[] {
  return Array.from(new Uint8Array(buffer));
}

describe('encodeUtf8', () => {
  it('writes the bytes TextEncoder writes, for one, two, three and four byte characters', () => {
    for (const text of ['ls -la\r', 'café', '€ 5', '😀 ok', '']) {
      expect(bytesOf(encodeUtf8(text))).toEqual(Array.from(new TextEncoder().encode(text)));
    }
  });

  it('writes a lone surrogate as the replacement character, as TextEncoder does', () => {
    const lone = '\ud800x';

    expect(bytesOf(encodeUtf8(lone))).toEqual(Array.from(new TextEncoder().encode(lone)));
  });
});
