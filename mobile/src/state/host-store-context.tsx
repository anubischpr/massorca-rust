import { createContext, useContext, useEffect, useState, useSyncExternalStore, type ReactNode } from 'react';

import type { MassOrcaBinding } from '../binding/types';
import { HostStore, type HostRecord } from './host-store';

const HostStoreContext = createContext<HostStore | null>(null);

/** Owns the host store for the app's lifetime; it closes the watches when the app unmounts. */
export function HostStoreProvider({ binding, children }: { binding: MassOrcaBinding; children: ReactNode }) {
  const [store] = useState(() => new HostStore(binding));
  useEffect(() => () => store.dispose(), [store]);
  return <HostStoreContext.Provider value={store}>{children}</HostStoreContext.Provider>;
}

export function useHostStore(): HostStore {
  const store = useContext(HostStoreContext);
  if (store === null) throw new Error('useHostStore must be used inside HostStoreProvider');
  return store;
}

/** The record of one host, re-rendered whenever the store changes it. */
export function useHostRecord(name: string): HostRecord | undefined {
  const store = useHostStore();
  return useSyncExternalStore(store.subscribe, () => store.getRecord(name));
}
