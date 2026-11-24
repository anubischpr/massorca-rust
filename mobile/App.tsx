import { SafeAreaProvider } from 'react-native-safe-area-context';

import { getBinding } from './src/binding';
import { BindingProvider } from './src/binding/binding-context';
import type { MassOrcaBinding } from './src/binding/types';
import { AppNavigator } from './src/navigation/app-navigator';

export interface AppProps {
  /** Tests pass a FakeMassOrca; the app uses the native binding behind the seam in src/binding. */
  readonly binding?: MassOrcaBinding;
}

export default function App({ binding }: AppProps) {
  const resolved = binding ?? getBinding();
  return (
    <SafeAreaProvider>
      <BindingProvider binding={resolved}>
        <AppNavigator />
      </BindingProvider>
    </SafeAreaProvider>
  );
}
