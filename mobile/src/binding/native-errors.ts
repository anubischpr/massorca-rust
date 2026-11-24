// What a failed call throws, as the app's FfiException. A generated FFI error keeps its case; anything else is internal.

import type * as Rust from '../generated/massorca_client_ffi';

import type { RustFfiError } from './native-shapes';
import { FfiException, type FfiError } from './types';

export function toException(error: unknown): FfiException {
  if (error instanceof FfiException) return error;
  if (isGeneratedError(error)) return new FfiException(ffiError(error), { cause: error });
  return new FfiException({ tag: 'Internal', message: 'The app hit an unexpected error.' }, { cause: error });
}

const FFI_TAGS: ReadonlySet<string> = new Set([
  'Unreachable',
  'IdentityMismatch',
  'NotPaired',
  'ProtocolMismatch',
  'InvalidOffer',
  'HostExists',
  'HostNotFound',
  'InvalidHostName',
  'Remote',
  'Storage',
  'Internal',
]);

function isGeneratedError(error: unknown): error is Rust.FfiError {
  return (
    typeof error === 'object' &&
    error !== null &&
    'inner' in error &&
    'tag' in error &&
    typeof error.tag === 'string' &&
    FFI_TAGS.has(error.tag)
  );
}

function ffiError(raw: RustFfiError): FfiError {
  switch (raw.tag) {
    case 'Unreachable':
      return { tag: 'Unreachable', message: raw.inner.message };
    case 'IdentityMismatch':
      return { tag: 'IdentityMismatch', message: raw.inner.message };
    case 'NotPaired':
      return { tag: 'NotPaired', message: raw.inner.message };
    case 'ProtocolMismatch':
      return {
        tag: 'ProtocolMismatch',
        message: raw.inner.message,
        supportedMin: raw.inner.supportedMin,
        supportedMax: raw.inner.supportedMax,
      };
    case 'InvalidOffer':
      return { tag: 'InvalidOffer', message: raw.inner.message };
    case 'HostExists':
      return { tag: 'HostExists', message: raw.inner.message };
    case 'HostNotFound':
      return { tag: 'HostNotFound', message: raw.inner.message };
    case 'InvalidHostName':
      return { tag: 'InvalidHostName', message: raw.inner.message };
    case 'Remote':
      return { tag: 'Remote', code: raw.inner.code, message: raw.inner.message };
    case 'Storage':
      return { tag: 'Storage', message: raw.inner.message };
    case 'Internal':
      return { tag: 'Internal', message: raw.inner.message };
  }
}
