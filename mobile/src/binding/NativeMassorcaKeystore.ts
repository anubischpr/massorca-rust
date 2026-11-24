// The Android Keystore module (decision 0080, section 3): host entries in app-private storage, and seal/unseal with a
// Keystore key that never leaves the device. Calls are synchronous because the Rust store makes them from inside a
// synchronous callback. Bytes cross as base64 strings.
import type { TurboModule } from 'react-native';
import { TurboModuleRegistry } from 'react-native';

export interface Spec extends TurboModule {
  /** The entry of a host (base64), or null when there is none. */
  readHost(name: string): string | null;
  writeHost(name: string, entry: string): void;
  removeHost(name: string): boolean;
  listHosts(): string[];
  seal(plain: string): string;
  unseal(sealed: string): string;
}

export default TurboModuleRegistry.getEnforcing<Spec>('MassorcaKeystore');
