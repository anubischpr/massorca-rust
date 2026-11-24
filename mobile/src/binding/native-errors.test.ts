import { toException } from './native-errors';
import { FfiException, type FfiError } from './types';

/** An error shaped the way the generated module throws an FfiError: a tag, and the case's fields in `inner`. */
function generated(tag: string, inner: object): Error {
  return Object.assign(new Error(tag), { tag, inner });
}

const cases: [string, Record<string, unknown>, FfiError][] = [
  ['Unreachable', { message: "VPS can't be reached." }, { tag: 'Unreachable', message: "VPS can't be reached." }],
  [
    'IdentityMismatch',
    { message: 'The key changed.', expected: 'sha256:a', actual: 'sha256:b' },
    { tag: 'IdentityMismatch', message: 'The key changed.' },
  ],
  ['NotPaired', { message: 'Pair again.' }, { tag: 'NotPaired', message: 'Pair again.' }],
  [
    'ProtocolMismatch',
    { message: 'Update the app.', supportedMin: 2, supportedMax: 3 },
    { tag: 'ProtocolMismatch', message: 'Update the app.', supportedMin: 2, supportedMax: 3 },
  ],
  ['InvalidOffer', { message: 'Not a code.' }, { tag: 'InvalidOffer', message: 'Not a code.' }],
  ['HostExists', { message: 'Already paired.' }, { tag: 'HostExists', message: 'Already paired.' }],
  ['HostNotFound', { message: 'No such host.' }, { tag: 'HostNotFound', message: 'No such host.' }],
  ['InvalidHostName', { message: 'Bad name.' }, { tag: 'InvalidHostName', message: 'Bad name.' }],
  [
    'Remote',
    { code: 'workspace.not_found', message: 'No such workspace.' },
    { tag: 'Remote', code: 'workspace.not_found', message: 'No such workspace.' },
  ],
  ['Storage', { message: 'The key could not be read.' }, { tag: 'Storage', message: 'The key could not be read.' }],
  ['Internal', { message: 'Unexpected data.' }, { tag: 'Internal', message: 'Unexpected data.' }],
];

describe('toException', () => {
  it.each(cases)('keeps the generated %s case as the app error', (tag, inner, expected) => {
    expect(toException(generated(tag, inner)).error).toEqual(expected);
  });

  it('keeps an absent version range of a protocol mismatch as undefined', () => {
    expect(toException(generated('ProtocolMismatch', { message: 'Update the app.' })).error).toEqual({
      tag: 'ProtocolMismatch',
      message: 'Update the app.',
      supportedMin: undefined,
      supportedMax: undefined,
    });
  });

  it('keeps the generated error as the cause', () => {
    const original = generated('NotPaired', { message: 'Pair again.' });

    expect(toException(original).cause).toBe(original);
  });

  it('passes an FfiException through unchanged', () => {
    const exception = new FfiException({ tag: 'Storage', message: 'x' });

    expect(toException(exception)).toBe(exception);
  });

  it('reports anything else as an internal failure that keeps the original error', () => {
    const original = new TypeError('boom');
    const exception = toException(original);

    expect(exception.error).toEqual({ tag: 'Internal', message: 'The app hit an unexpected error.' });
    expect(exception.cause).toBe(original);
  });

  it('treats a tag this build does not know as an internal failure', () => {
    expect(toException(generated('Mystery', { message: 'x' })).error.tag).toBe('Internal');
  });
});
