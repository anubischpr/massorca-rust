import type * as Rust from '../generated/massorca_client_ffi';

import { KeystoreHostStore, type KeystoreModule } from './host-store';
import { createNativeBinding } from './native';
import type { MassOrcaBinding } from './types';

/** What hosts call this phone. */
const DEVICE_NAME = 'Android phone';

let binding: MassOrcaBinding | undefined;

/**
 * The app's binding. The first call loads the Rust library and creates the client; later calls return the same one.
 * Loading waits for this call because the library exists only on the device, not in Jest.
 */
export function getBinding(): MassOrcaBinding {
  if (binding === undefined) {
    binding = connectNative();
  }
  return binding;
}

function connectNative(): MassOrcaBinding {
  // Required here, not imported: loading the entry installs the native library on the device.
  const rust: typeof Rust = require('../index');
  const keystore: KeystoreModule = require('./NativeMassorcaKeystore').default;
  const { version }: { version: string } = require('../../package.json');
  return createNativeBinding(rust, {
    store: new KeystoreHostStore(keystore),
    deviceName: DEVICE_NAME,
    appVersion: version,
  });
}
