import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { useEffect, useState } from 'react';
import { ActivityIndicator, Linking, StyleSheet, Text, View } from 'react-native';

import { Card } from '../components/card';
import { StatePage } from '../components/state-page';
import { CANT_REACH_LINE } from '../copy/hosts';
import {
  PROTOCOL_BACK_TO_HOSTS,
  PROTOCOL_CHECK_AGAIN,
  PROTOCOL_CHECK_FAILED,
  PROTOCOL_OPEN_STORE,
  protocolGateText,
  protocolRangeText,
} from '../copy/protocol';
import type { RootStackParamList } from '../navigation/routes';
import { APP_PROTOCOL } from '../state/app-protocol';
import { connectionProblemOf } from '../state/connection-problem';
import { useHostRecord, useHostStore } from '../state/host-store-context';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';

/** The Android package name (applicationId in mobile/android/app/build.gradle). */
const PLAY_STORE_URL = 'market://details?id=dev.massorca.app';

type Props = NativeStackScreenProps<RootStackParamList, 'ProtocolGate'>;

/** M-SH-02: blocks one host whose protocol this app does not speak, and says which side must update. */
export function ProtocolGateScreen({ navigation, route }: Props) {
  const { host } = route.params;
  const store = useHostStore();
  const palette = usePalette();
  const record = useHostRecord(host);
  const [checking, setChecking] = useState(false);
  const connection = record?.connection;

  // Opened after a failed connect, the gate shows that failure; it checks again only when asked.
  useEffect(() => {
    if (store.getRecord(host) === undefined) store.ensure(host);
  }, [host, store]);

  useEffect(() => {
    if (connection?.status === 'connected') navigation.replace('Workspaces', { host });
  }, [connection?.status, host, navigation]);

  async function checkAgain() {
    setChecking(true);
    await store.retry(host);
    setChecking(false);
  }

  function openStore() {
    Linking.openURL(PLAY_STORE_URL).catch(() => undefined);
  }

  if (connection?.status !== 'failed') {
    return (
      <View style={[styles.checking, { backgroundColor: palette['surface.canvas'] }]}>
        <ActivityIndicator accessibilityLabel="Checking the protocol" color={palette['text.secondary']} />
      </View>
    );
  }

  const problem = connectionProblemOf(connection.error);
  if (problem.kind !== 'protocol') {
    return (
      <StatePage
        title={PROTOCOL_CHECK_FAILED(host)}
        line={problem.kind === 'unreachable' ? CANT_REACH_LINE(host) : undefined}
        detail={connection.error.message}
        actions={[
          { label: PROTOCOL_CHECK_AGAIN, onPress: checkAgain, variant: 'primary', busy: checking },
          { label: PROTOCOL_BACK_TO_HOSTS, onPress: () => navigation.navigate('Home'), variant: 'secondary' },
        ]}
      />
    );
  }

  const text = protocolGateText(problem.side, host, problem.hostMin, problem.hostMax);
  const primary =
    problem.side === 'app'
      ? { label: PROTOCOL_OPEN_STORE, onPress: openStore, variant: 'primary' as const }
      : { label: PROTOCOL_CHECK_AGAIN, onPress: checkAgain, variant: 'primary' as const, busy: checking };

  return (
    <StatePage
      title={text.title}
      line={text.body}
      actions={[primary, { label: PROTOCOL_BACK_TO_HOSTS, onPress: () => navigation.navigate('Home'), variant: 'secondary' }]}
    >
      <Card>
        <View style={styles.version}>
          <Text style={[styles.label, { color: palette['text.secondary'] }]}>This app</Text>
          <Text style={[styles.value, { color: palette['text.primary'] }]}>
            {protocolRangeText(APP_PROTOCOL.min, APP_PROTOCOL.max)}
          </Text>
        </View>
        <View style={styles.version}>
          <Text style={[styles.label, { color: palette['text.secondary'] }]}>{host}</Text>
          <Text style={[styles.value, { color: palette['text.primary'] }]}>
            {protocolRangeText(problem.hostMin, problem.hostMax)}
          </Text>
        </View>
      </Card>
    </StatePage>
  );
}

const styles = StyleSheet.create({
  checking: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
  },
  version: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacingTokens['space.3'],
    minHeight: spacingTokens['space.6'],
    alignItems: 'center',
  },
  label: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
  },
  value: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
    fontWeight: '600',
  },
});
