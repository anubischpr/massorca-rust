import type { FfiError } from '../binding/types';
import { updateSideFor, type UpdateSide } from './app-protocol';

export type ConnectionProblem =
  | { readonly kind: 'unreachable' }
  | { readonly kind: 'sign-in'; readonly message: string }
  | {
      readonly kind: 'protocol';
      readonly side: UpdateSide;
      readonly hostMin: number | undefined;
      readonly hostMax: number | undefined;
    }
  | { readonly kind: 'other'; readonly message: string };

/** Sorts a failed connect into what the host card and the banner say, and which action fits. */
export function connectionProblemOf(error: FfiError): ConnectionProblem {
  switch (error.tag) {
    case 'Unreachable':
      return { kind: 'unreachable' };
    case 'NotPaired':
    case 'IdentityMismatch':
    case 'Storage':
      return { kind: 'sign-in', message: error.message };
    case 'ProtocolMismatch':
      return {
        kind: 'protocol',
        side: updateSideFor({ min: error.supportedMin, max: error.supportedMax }),
        hostMin: error.supportedMin,
        hostMax: error.supportedMax,
      };
    default:
      return { kind: 'other', message: error.message };
  }
}
