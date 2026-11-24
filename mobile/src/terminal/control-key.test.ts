import { applyControl } from './control-key';

describe('applyControl', () => {
  it('turns a letter into its control code, in either case', () => {
    expect(applyControl('c')).toBe('\u0003');
    expect(applyControl('A')).toBe('\u0001');
    expect(applyControl('z')).toBe('\u001a');
  });

  it('turns the punctuation of the control range into its code, and space and ? into NUL and DEL', () => {
    expect(applyControl('[')).toBe('\u001b');
    expect(applyControl('@')).toBe('\u0000');
    expect(applyControl(' ')).toBe('\u0000');
    expect(applyControl('?')).toBe('\u007f');
  });

  it('types everything else as it is: digits, text, non-ASCII and more than one character', () => {
    expect(applyControl('1')).toBe('1');
    expect(applyControl('é')).toBe('é');
    expect(applyControl('ls')).toBe('ls');
    expect(applyControl('\r')).toBe('\r');
    expect(applyControl('')).toBe('');
  });
});
