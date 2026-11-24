import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { useMemo, useState } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { useBinding } from '../binding/binding-context';
import { FfiException, type FfiError, type OfferSummary } from '../binding/types';
import { ActionButton } from '../components/action-button';
import { Card } from '../components/card';
import { ConnectionBanner } from '../components/connection-banner';
import { StatePage } from '../components/state-page';
import { TextField } from '../components/text-field';
import { RETRY } from '../copy/connection';
import {
  ADDRESS_LABEL,
  BACK_TO_HOME,
  CANCEL,
  CONFIRM_HEADING,
  CONFIRM_LINE,
  HOST_NAME_HELP,
  HOST_NAME_INVALID,
  HOST_NAME_LABEL,
  INVALID_CODE,
  PAIR,
  PAIR_FAILED_LINE,
  pairFailed,
} from '../copy/pairing';
import type { RootStackParamList } from '../navigation/routes';
import { isValidHostName, suggestHostName } from '../state/host-name';
import { useHostStore } from '../state/host-store-context';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';

type Props = NativeStackScreenProps<RootStackParamList, 'ConfirmPairing'>;

/** M-PA-04: confirm the desktop before the pairing secret is used. */
export function ConfirmPairingScreen({ navigation, route }: Props) {
  const binding = useBinding();
  const store = useHostStore();
  const palette = usePalette();
  const offer = useMemo(() => parseOffer(() => binding.parseOffer(route.params.offer)), [binding, route.params.offer]);
  const [name, setName] = useState(() => (offer === undefined ? '' : suggestHostName(offer.server)));
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<FfiError | undefined>(undefined);

  if (offer === undefined) {
    return (
      <StatePage
        title={INVALID_CODE}
        actions={[{ label: BACK_TO_HOME, onPress: () => navigation.popToTop() }]}
      />
    );
  }

  const nameValid = isValidHostName(name);

  async function pair() {
    setBusy(true);
    setFailure(undefined);
    try {
      const host = await binding.pair(route.params.offer, name);
      store.ensure(host.name);
      navigation.popToTop();
    } catch (error) {
      setFailure(error instanceof FfiException ? error.error : { tag: 'Internal', message: String(error) });
      setBusy(false);
    }
  }

  return (
    <ScrollView style={{ backgroundColor: palette['surface.canvas'] }} contentContainerStyle={styles.content}>
      <Text accessibilityRole="header" style={[styles.heading, { color: palette['text.primary'] }]}>
        {CONFIRM_HEADING}
      </Text>
      <Card>
        <Text style={[styles.title, { color: palette['text.primary'] }]}>{offer.server}</Text>
        <Text style={[styles.caption, { color: palette['text.secondary'] }]}>{ADDRESS_LABEL}</Text>
        <Text style={[styles.mono, { color: palette['text.primary'] }]}>{offer.addresses.join(', ')}</Text>
      </Card>
      <TextField
        label={HOST_NAME_LABEL}
        value={name}
        onChangeText={setName}
        helper={HOST_NAME_HELP}
        error={name !== '' && !nameValid ? HOST_NAME_INVALID : undefined}
      />
      <Text style={[styles.line, { color: palette['text.secondary'] }]}>{CONFIRM_LINE}</Text>
      {failure !== undefined && (
        <ConnectionBanner
          tone="danger"
          message={`${pairFailed(failure.message)}${failure.tag === 'Unreachable' ? ` ${PAIR_FAILED_LINE}` : ''}`}
          actions={[{ label: RETRY, onPress: pair }]}
        />
      )}
      <View style={styles.pinned}>
        <ActionButton label={PAIR} variant="primary" busy={busy} disabled={!nameValid || busy} onPress={pair} />
        <ActionButton label={CANCEL} variant="secondary" disabled={busy} onPress={() => navigation.goBack()} />
      </View>
    </ScrollView>
  );
}

function parseOffer(parse: () => OfferSummary): OfferSummary | undefined {
  try {
    return parse();
  } catch {
    return undefined;
  }
}

const styles = StyleSheet.create({
  content: {
    padding: spacingTokens['space.4'],
    gap: spacingTokens['space.3'],
  },
  heading: {
    fontSize: typeTokens['a-heading'].size,
    lineHeight: typeTokens['a-heading'].lineHeight,
    fontWeight: '600',
  },
  title: {
    fontSize: typeTokens['a-title'].size,
    lineHeight: typeTokens['a-title'].lineHeight,
    fontWeight: '600',
  },
  caption: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
  },
  mono: {
    fontSize: typeTokens['a-mono'].size,
    lineHeight: typeTokens['a-mono'].lineHeight,
    fontFamily: 'monospace',
  },
  line: {
    fontSize: typeTokens['a-body-sm'].size,
    lineHeight: typeTokens['a-body-sm'].lineHeight,
  },
  pinned: {
    gap: spacingTokens['space.2'],
    paddingTop: spacingTokens['space.2'],
  },
});
