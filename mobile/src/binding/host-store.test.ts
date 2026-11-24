import { fromBase64, toBase64 } from './base64';
import { KeystoreHostStore, type KeystoreModule } from './host-store';

function bytesOf(values: readonly number[]): ArrayBuffer {
  return Uint8Array.from(values).buffer;
}

function valuesOf(buffer: ArrayBuffer | undefined): number[] | undefined {
  return buffer === undefined ? undefined : Array.from(new Uint8Array(buffer));
}

/** A Keystore stand-in: entries in a map, and a seal that only its own unseal reads back. */
class FakeKeystore implements KeystoreModule {
  readonly entries = new Map<string, string>();
  private readonly sealed = new Set<string>();
  failure: Error | undefined;

  readHost(name: string): string | null {
    this.fail();
    return this.entries.get(name) ?? null;
  }

  writeHost(name: string, entry: string): void {
    this.fail();
    this.entries.set(name, entry);
  }

  removeHost(name: string): boolean {
    this.fail();
    return this.entries.delete(name);
  }

  listHosts(): string[] {
    this.fail();
    return [...this.entries.keys()];
  }

  seal(plain: string): string {
    this.fail();
    const sealed = flip(plain);
    this.sealed.add(sealed);
    return sealed;
  }

  unseal(sealed: string): string {
    this.fail();
    if (!this.sealed.has(sealed)) throw new Error('the key does not open this value');
    return flip(sealed);
  }

  private fail(): void {
    if (this.failure !== undefined) throw this.failure;
  }
}

/** Inverts every byte, so a sealed value never reads as the plain one. */
function flip(base64: string): string {
  return toBase64(Uint8Array.from(new Uint8Array(fromBase64(base64)), (byte) => 255 - byte).buffer);
}

describe('KeystoreHostStore', () => {
  it('returns no entry for a host that was never written', () => {
    const store = new KeystoreHostStore(new FakeKeystore());

    expect(store.readHost('home')).toBeUndefined();
  });

  it('writes an entry as base64 and reads back the same bytes', () => {
    const keystore = new FakeKeystore();
    const store = new KeystoreHostStore(keystore);

    store.writeHost('home', bytesOf([1, 2, 3]));

    expect(keystore.entries.get('home')).toBe('AQID');
    expect(valuesOf(store.readHost('home'))).toEqual([1, 2, 3]);
  });

  it('replaces an entry whole and removes it', () => {
    const keystore = new FakeKeystore();
    const store = new KeystoreHostStore(keystore);
    store.writeHost('home', bytesOf([1]));
    store.writeHost('home', bytesOf([9, 9]));

    expect(valuesOf(store.readHost('home'))).toEqual([9, 9]);
    expect(store.removeHost('home')).toBe(true);
    expect(store.removeHost('home')).toBe(false);
    expect(store.readHost('home')).toBeUndefined();
  });

  it('lists the names that have an entry', () => {
    const store = new KeystoreHostStore(new FakeKeystore());
    store.writeHost('home', bytesOf([1]));
    store.writeHost('office', bytesOf([2]));

    expect(store.listHosts()).toEqual(['home', 'office']);
  });

  it('seals and unseals bytes through the Keystore', () => {
    const store = new KeystoreHostStore(new FakeKeystore());
    const sealed = store.seal(bytesOf([0x10, 0x20]));

    expect(valuesOf(sealed)).toEqual([0xef, 0xdf]);
    expect(valuesOf(store.unseal(sealed))).toEqual([0x10, 0x20]);
  });

  it('fails when the Keystore cannot open a value', () => {
    const store = new KeystoreHostStore(new FakeKeystore());

    expect(() => store.unseal(bytesOf([1, 2, 3]))).toThrow('the key does not open this value');
  });

  it('passes a failure of the native module on, without turning it into an entry', () => {
    const keystore = new FakeKeystore();
    const store = new KeystoreHostStore(keystore);
    keystore.failure = new Error('storage is full');

    expect(() => store.writeHost('home', bytesOf([1]))).toThrow('storage is full');
    expect(() => store.readHost('home')).toThrow('storage is full');
  });
});
