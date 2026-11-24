import type { MassOrcaBinding } from './types';

/**
 * The seam where the native binding plugs in. The generated uniffi-bindgen-react-native module (next task) replaces
 * this body. Until the native library is built the app has no binding, so starting it fails here; tests pass
 * a FakeMassOrca to App instead.
 */
export function getBinding(): MassOrcaBinding {
  throw new Error('native binding not built');
}
