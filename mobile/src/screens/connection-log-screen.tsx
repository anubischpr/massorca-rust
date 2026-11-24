import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { useEffect } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { Card } from '../components/card';
import { Overline } from '../components/overline';
import { StateBadge, type BadgeTone } from '../components/state-badge';
import { lastContactLine, LOG_EMPTY, LOG_HEADING, NO_CONTACT, STATUS_WORDS, TRY_STEPS, WHAT_TO_TRY } from '../copy/connection';
import type { RootStackParamList } from '../navigation/routes';
import { formatLogClock } from '../state/connection-log';
import type { HostRecord } from '../state/host-store';
import { useHostRecord, useHostStore } from '../state/host-store-context';
import { formatAge } from '../state/relative-age';
import { useNow } from '../state/use-now';
import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens } from '../theme/tokens.generated';

type Props = NativeStackScreenProps<RootStackParamList, 'ConnectionLog'>;

/** X-DG-01: the host's connection status, what to try, and the log of its connection (the last 50 lines). */
export function ConnectionLogScreen({ route }: Props) {
  const { host } = route.params;
  const store = useHostStore();
  const palette = usePalette();
  const record = useHostRecord(host);
  const now = useNow(60_000);

  useEffect(() => {
    if (store.getRecord(host) === undefined) store.ensure(host);
  }, [host, store]);

  const status = statusOf(record);
  const lastContact = record?.agents?.lastContactMs;
  const contact = lastContact === undefined ? NO_CONTACT : lastContactLine(formatAge(now - lastContact));
  const log = record?.log ?? [];

  return (
    <ScrollView style={{ backgroundColor: palette['surface.canvas'] }} contentContainerStyle={styles.content}>
      <Text accessibilityRole="header" style={[styles.host, { color: palette['text.primary'] }]}>
        {host}
      </Text>
      <Card>
        <View style={styles.statusRow}>
          <StateBadge label={status.label} tone={status.tone} />
          <Text style={[styles.line, { color: palette['text.secondary'] }]}>{contact}</Text>
        </View>
      </Card>
      <Overline>{WHAT_TO_TRY}</Overline>
      {TRY_STEPS(host).map((step, index) => (
        <Text key={step} style={[styles.line, { color: palette['text.primary'] }]}>
          {`${index + 1}. ${step}`}
        </Text>
      ))}
      <Overline>{LOG_HEADING}</Overline>
      <View style={[styles.well, { backgroundColor: palette['surface.sunken'], borderColor: palette['border.subtle'] }]}>
        {log.length === 0 ? (
          <Text style={[styles.logLine, { color: palette['text.tertiary'] }]}>{LOG_EMPTY}</Text>
        ) : (
          log.map((line, index) => (
            <Text key={`${index}`} style={[styles.logLine, { color: palette['text.secondary'] }]}>
              {`${formatLogClock(line.atMs)}  ${line.text}`}
            </Text>
          ))
        )}
      </View>
    </ScrollView>
  );
}

function statusOf(record: HostRecord | undefined): { readonly label: string; readonly tone: BadgeTone } {
  const connection = record?.connection;
  if (connection === undefined || connection.status === 'connecting') {
    return { label: STATUS_WORDS.connecting, tone: 'info' };
  }
  if (connection.status === 'failed') return { label: STATUS_WORDS.failed, tone: 'danger' };
  if (record?.agents?.link.tag === 'Unverifiable') return { label: STATUS_WORDS.warning, tone: 'warning' };
  return { label: STATUS_WORDS.ok, tone: 'success' };
}

const styles = StyleSheet.create({
  content: {
    padding: spacingTokens['space.4'],
    gap: spacingTokens['space.3'],
  },
  host: {
    fontSize: typeTokens['a-heading'].size,
    lineHeight: typeTokens['a-heading'].lineHeight,
    fontWeight: '600',
  },
  statusRow: {
    flexDirection: 'row',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: spacingTokens['space.3'],
  },
  line: {
    fontSize: typeTokens['a-body-sm'].size,
    lineHeight: typeTokens['a-body-sm'].lineHeight,
  },
  well: {
    borderWidth: 1,
    borderRadius: radiusTokens['radius.lg'],
    padding: spacingTokens['space.3'],
    gap: spacingTokens['space.1'],
  },
  logLine: {
    fontSize: typeTokens['a-mono'].size,
    lineHeight: typeTokens['a-mono'].lineHeight,
    fontFamily: 'monospace',
  },
});
