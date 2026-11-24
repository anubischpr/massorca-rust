import { useFocusEffect } from '@react-navigation/native';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { useCallback, useState } from 'react';

import { useBinding } from '../binding/binding-context';
import type { RootStackParamList } from '../navigation/routes';
import { ConnectHint } from './connect-hint';
import { PlaceholderScreen } from './placeholder-screen';

type Props = NativeStackScreenProps<RootStackParamList, 'Home'>;

/** Home (M-HM-01). With no host paired it shows the connect hint; the host cards come in a later task. */
export function HomeScreen({ navigation }: Props) {
  const binding = useBinding();
  const [hostCount, setHostCount] = useState(() => binding.hosts().length);

  // Pairing happens on another screen, so the count is read again each time Home comes back.
  useFocusEffect(
    useCallback(() => {
      setHostCount(binding.hosts().length);
    }, [binding]),
  );

  if (hostCount === 0) {
    return (
      <ConnectHint
        onPair={() => navigation.navigate('Connect', { mode: 'scan' })}
        onPaste={() => navigation.navigate('Connect', { mode: 'paste' })}
      />
    );
  }
  return <PlaceholderScreen title="Home" />;
}
