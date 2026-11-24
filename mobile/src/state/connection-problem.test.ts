import type { FfiError } from '../binding/types';
import { updateSideFor } from './app-protocol';
import { connectionProblemOf } from './connection-problem';

describe('connectionProblemOf', () => {
  it('maps a refused network to unreachable', () => {
    expect(connectionProblemOf({ tag: 'Unreachable', message: 'unreachable' })).toEqual({ kind: 'unreachable' });
  });

  it('maps revoked and changed trust to sign-in with the service sentence', () => {
    expect(connectionProblemOf({ tag: 'NotPaired', message: 'home-pc no longer trusts this phone.' })).toEqual({
      kind: 'sign-in',
      message: 'home-pc no longer trusts this phone.',
    });
    expect(connectionProblemOf({ tag: 'IdentityMismatch', message: 'changed' })).toEqual({
      kind: 'sign-in',
      message: 'changed',
    });
  });

  it('keeps the protocol range the service speaks', () => {
    const error: FfiError = {
      tag: 'ProtocolMismatch',
      message: 'refused',
      supportedMin: 2,
      supportedMax: 3,
    };

    expect(connectionProblemOf(error)).toEqual({ kind: 'protocol', side: 'app', hostMin: 2, hostMax: 3 });
  });

  it('sends everything else to the generic failure', () => {
    expect(connectionProblemOf({ tag: 'Remote', code: 'path.not_found', message: 'gone' })).toEqual({
      kind: 'other',
      message: 'gone',
    });
  });
});

describe('updateSideFor', () => {
  it('names the host when it speaks only versions below the app minimum', () => {
    expect(updateSideFor({ min: 0, max: 0 })).toBe('host');
  });

  it('names the app when the host needs versions above the app maximum', () => {
    expect(updateSideFor({ min: 2, max: 3 })).toBe('app');
  });

  it('cannot tell which side when the host range is unknown', () => {
    expect(updateSideFor({ min: undefined, max: undefined })).toBe('unknown');
  });

  it('is not a mismatch when the ranges overlap', () => {
    expect(updateSideFor({ min: 1, max: 4 })).toBe('unknown');
  });
});
