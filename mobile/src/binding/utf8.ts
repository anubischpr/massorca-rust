// UTF-8 bytes of text, written out because React Native does not guarantee TextEncoder (base64.ts is written out for the
// same reason).

/** The UTF-8 bytes of `text`; a lone surrogate becomes U+FFFD, as TextEncoder writes it. */
export function encodeUtf8(text: string): ArrayBuffer {
  const out: number[] = [];
  for (const char of text) {
    const point = char.codePointAt(0) ?? 0xfffd;
    const code = point >= 0xd800 && point <= 0xdfff ? 0xfffd : point;
    if (code < 0x80) {
      out.push(code);
    } else if (code < 0x800) {
      out.push(0xc0 + Math.floor(code / 0x40), 0x80 + (code % 0x40));
    } else if (code < 0x10000) {
      out.push(0xe0 + Math.floor(code / 0x1000), 0x80 + (Math.floor(code / 0x40) % 0x40), 0x80 + (code % 0x40));
    } else {
      out.push(
        0xf0 + Math.floor(code / 0x40000),
        0x80 + (Math.floor(code / 0x1000) % 0x40),
        0x80 + (Math.floor(code / 0x40) % 0x40),
        0x80 + (code % 0x40),
      );
    }
  }
  return new Uint8Array(out).buffer;
}
