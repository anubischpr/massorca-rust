import { useFocusEffect } from '@react-navigation/native';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { useCallback, useEffect, useState } from 'react';
import { RefreshControl, ScrollView, StyleSheet, View } from 'react-native';

import type { HostSummary } from '../binding/types';
import { useBinding } from '../binding/binding-context';
import { ActionButton } from '../components/action-button';
import { Overline } from '../components/overline';
import { PAIR_ACTION, HOSTS_HEADING } from '../copy/hosts';
import type { RootStackParamList } from '../navigation/routes';
import { useHostStore } from '../state/host-store-context';
import { usePalette } from '../theme/palette';
import { spacingTokens } from '../theme/tokens.generated';
import { ConnectHint } from './connect-hint';
import { HostCard } from './host-card';

type Props = NativeStackScreenProps<RootStackParamList, 'Home'>;

/** Home (M-HM-01): the paired hosts as cards; with no host, the connect hint (M-PA-01). */
export function HomeScreen({ navigation }: Props) {
  const binding = useBinding();
  const store = useHostStore();
  const palette = usePalette();
  const [hosts, setHosts] = useState<readonly HostSummary[]>(() => binding.hosts());
  const [refreshing, setRefreshing] = useState(false);

  // Pairing happens on another screen, so the host list is read again each time Home comes back.
  useFocusEffect(
    useCallback(() => {
      setHosts(binding.hosts());
    }, [binding]),
  );

  useEffect(() => {
    for (const host of hosts) store.ensure(host.name);
  }, [hosts, store]);

  if (hosts.length === 0) {
    return (
      <ConnectHint
        onPair={() => navigation.navigate('Connect', { mode: 'scan' })}
        onPaste={() => navigation.navigate('Connect', { mode: 'paste' })}
      />
    );
  }

  async function refresh() {
    setRefreshing(true);
    await Promise.all(hosts.map((host) => store.retry(host.name)));
    setRefreshing(false);
  }

  return (
    <ScrollView
      style={{ backgroundColor: palette['surface.canvas'] }}
      contentContainerStyle={styles.content}
      refreshControl={<RefreshControl refreshing={refreshing} onRefresh={refresh} />}
    >
      <View style={styles.heading}>
        <Overline>{HOSTS_HEADING}</Overline>
        <ActionButton label={PAIR_ACTION} variant="ghost" onPress={() => navigation.navigate('Connect', { mode: 'scan' })} />
      </View>
      {hosts.map((host) => (
        <HostCard
          key={host.name}
          host={host}
          onTap={(tap) => {
            if (tap === 'protocol') navigation.navigate('ProtocolGate', { host: host.name });
            else if (tap === 'pair') navigation.navigate('Connect', { mode: 'scan' });
            else navigation.navigate('Workspaces', { host: host.name });
          }}
          onAction={(kind) => {
            if (kind === 'retry') store.retry(host.name);
            else navigation.navigate('Connect', { mode: 'scan' });
          }}
        />
      ))}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  content: {
    padding: spacingTokens['space.4'],
    gap: spacingTokens['space.3'],
  },
  heading: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
});
