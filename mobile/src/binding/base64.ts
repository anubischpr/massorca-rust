// Base64 (RFC 4648, padded) for the Keystore module, which moves bytes as strings. Written here because React Native
// does not guarantee btoa or atob.

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
const PAD = '=';

export function toBase64(bytes: ArrayBuffer): string {
  const input = new Uint8Array(bytes);
  const out: string[] = [];
  for (let start = 0; start < input.length; start += 3) {
    const count = Math.min(3, input.length - start);
    const triple = (input[start] ?? 0) * 65536 + (input[start + 1] ?? 0) * 256 + (input[start + 2] ?? 0);
    for (let digit = 0; digit < 4; digit++) {
      out.push(digit <= count ? ALPHABET.charAt(Math.floor(triple / 2 ** (18 - 6 * digit)) % 64) : PAD);
    }
  }
  return out.join('');
}

export function fromBase64(text: string): ArrayBuffer {
  if (text.length % 4 !== 0) throw new Error('base64 text must have a length that is a multiple of 4');
  const out: number[] = [];
  for (let start = 0; start < text.length; start += 4) {
    const last = start + 4 === text.length;
    const pad = text.endsWith('==', start + 4) ? 2 : text.endsWith('=', start + 4) ? 1 : 0;
    if (pad > 0 && !last) throw new Error('base64 padding is only allowed at the end');
    let triple = 0;
    for (let digit = 0; digit < 4 - pad; digit++) {
      const value = ALPHABET.indexOf(text.charAt(start + digit));
      if (value < 0) throw new Error('base64 text has a character outside the alphabet');
      triple = triple * 64 + value;
    }
    triple *= 64 ** pad;
    out.push(Math.floor(triple / 65536) % 256);
    if (pad < 2) out.push(Math.floor(triple / 256) % 256);
    if (pad < 1) out.push(triple % 256);
  }
  return new Uint8Array(out).buffer;
}
