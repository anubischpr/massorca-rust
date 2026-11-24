// The Rust client's HostStore (crates/client-ffi/src/store.rs) on Android: the entries and the Keystore calls come from
// the native module, and bytes cross it as base64.
import type { HostStore } from '../generated/massorca_client_ffi';

import { fromBase64, toBase64 } from './base64';

/** The part of the Keystore module (NativeMassorcaKeystore.ts) that the store uses. */
export interface KeystoreModule {
  readHost(name: string): string | null;
  writeHost(name: string, entry: string): void;
  removeHost(name: string): boolean;
  listHosts(): string[];
  seal(plain: string): string;
  unseal(sealed: string): string;
}

export class KeystoreHostStore implements HostStore {
  private readonly keystore: KeystoreModule;

  constructor(keystore: KeystoreModule) {
    this.keystore = keystore;
  }

  readHost(name: string): ArrayBuffer | undefined {
    const entry = this.keystore.readHost(name);
    return entry === null ? undefined : fromBase64(entry);
  }

  writeHost(name: string, entry: ArrayBuffer): void {
    this.keystore.writeHost(name, toBase64(entry));
  }

  removeHost(name: string): boolean {
    return this.keystore.removeHost(name);
  }

  listHosts(): string[] {
    return this.keystore.listHosts();
  }

  seal(plain: ArrayBuffer): ArrayBuffer {
    return fromBase64(this.keystore.seal(toBase64(plain)));
  }

  unseal(sealed: ArrayBuffer): ArrayBuffer {
    return fromBase64(this.keystore.unseal(toBase64(sealed)));
  }
}
