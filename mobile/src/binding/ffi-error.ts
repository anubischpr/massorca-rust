// Mirrors FfiError::code and FfiError::is_transient in crates/client-ffi/src/error.rs, so fakes and screens classify
// errors the way Rust does.

import type { FfiError } from './types';

/** The service code that means "try again later" (massorca_protocol::codes::SERVICE_SHUTTING_DOWN). */
const SERVICE_SHUTTING_DOWN = 'service.shutting_down';

export function errorCode(error: FfiError): string {
  switch (error.tag) {
    case 'Unreachable':
      return 'unreachable';
    case 'IdentityMismatch':
      return 'identity_mismatch';
    case 'NotPaired':
      return 'not_paired';
    case 'ProtocolMismatch':
      return 'protocol_mismatch';
    case 'InvalidOffer':
      return 'invalid_offer';
    case 'HostExists':
      return 'host_exists';
    case 'HostNotFound':
      return 'host_not_found';
    case 'InvalidHostName':
      return 'invalid_host_name';
    case 'Remote':
      return error.code;
    case 'Storage':
      return 'storage';
    case 'Internal':
      return 'internal';
  }
}

/** Whether trying again can help: the app shows "Retry" for these and "Pair again" otherwise. */
export function isTransient(error: FfiError): boolean {
  switch (error.tag) {
    case 'Unreachable':
      return true;
    case 'Remote':
      return error.code === SERVICE_SHUTTING_DOWN;
    default:
      return false;
  }
}

/** Builds the case for a stable code, so a scripted failure can name only the code. Unknown codes are `Remote`. */
export function errorFromCode(code: string, message: string): FfiError {
  switch (code) {
    case 'unreachable':
      return { tag: 'Unreachable', message };
    case 'identity_mismatch':
      return { tag: 'IdentityMismatch', message };
    case 'not_paired':
      return { tag: 'NotPaired', message };
    case 'protocol_mismatch':
      return { tag: 'ProtocolMismatch', message, supportedMin: undefined, supportedMax: undefined };
    case 'invalid_offer':
      return { tag: 'InvalidOffer', message };
    case 'host_exists':
      return { tag: 'HostExists', message };
    case 'host_not_found':
      return { tag: 'HostNotFound', message };
    case 'invalid_host_name':
      return { tag: 'InvalidHostName', message };
    case 'storage':
      return { tag: 'Storage', message };
    case 'internal':
      return { tag: 'Internal', message };
    default:
      return { tag: 'Remote', code, message };
  }
}
