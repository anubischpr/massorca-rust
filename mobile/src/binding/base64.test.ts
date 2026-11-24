import { fromBase64, toBase64 } from './base64';

function bytesOf(values: readonly number[]): ArrayBuffer {
  return Uint8Array.from(values).buffer;
}

function valuesOf(buffer: ArrayBuffer): number[] {
  return Array.from(new Uint8Array(buffer));
}

// RFC 4648, section 10.
const vectors: readonly [string, string][] = [
  ['', ''],
  ['f', 'Zg=='],
  ['fo', 'Zm8='],
  ['foo', 'Zm9v'],
  ['foob', 'Zm9vYg=='],
  ['fooba', 'Zm9vYmE='],
  ['foobar', 'Zm9vYmFy'],
];

describe('base64', () => {
  it.each(vectors)('encodes the bytes of %j as %j', (text, encoded) => {
    const bytes = Array.from(text, (char) => char.charCodeAt(0));

    expect(toBase64(bytesOf(bytes))).toBe(encoded);
    expect(valuesOf(fromBase64(encoded))).toEqual(bytes);
  });

  it('round-trips every byte value at every length up to five', () => {
    for (let length = 0; length <= 5; length++) {
      for (let value = 0; value < 256; value++) {
        const bytes = Array.from({ length }, (_, index) => (value + index * 37) % 256);
        expect(valuesOf(fromBase64(toBase64(bytesOf(bytes))))).toEqual(bytes);
      }
    }
  });

  it('refuses text that is not base64', () => {
    expect(() => fromBase64('Zm9')).toThrow('multiple of 4');
    expect(() => fromBase64('Zm!v')).toThrow('outside the alphabet');
    expect(() => fromBase64('Z=g=')).toThrow('outside the alphabet');
    expect(() => fromBase64('Zg==Zm8=')).toThrow('only allowed at the end');
  });
});
