import { createContext, useContext, type ReactNode } from 'react';

import type { MassOrcaBinding } from './types';

const BindingContext = createContext<MassOrcaBinding | null>(null);

export function BindingProvider({ binding, children }: { binding: MassOrcaBinding; children: ReactNode }) {
  return <BindingContext.Provider value={binding}>{children}</BindingContext.Provider>;
}

/** The binding of the app, or a fake in tests. Throws when rendered outside a BindingProvider. */
export function useBinding(): MassOrcaBinding {
  const binding = useContext(BindingContext);
  if (binding === null) throw new Error('useBinding must be used inside BindingProvider');
  return binding;
}
