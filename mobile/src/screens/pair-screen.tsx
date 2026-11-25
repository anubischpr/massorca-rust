import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { useState } from 'react';
import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { useBinding } from '../binding/binding-context';
import { ActionButton } from '../components/action-button';
import { SegmentedControl } from '../components/segmented-control';
import { TextField } from '../components/text-field';
import {
  CONNECT,
  INVALID_CODE,
  PAIRING_CODE_LABEL,
  PASTE_CODE_INSTEAD,
  PASTE_TAB,
  SCAN_LATER,
  SCAN_TAB,
} from '../copy/pairing';
import type { PairMode, RootStackParamList } from '../navigation/routes';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';

type Props = NativeStackScreenProps<RootStackParamList, 'Connect'>;

/** M-PA-02: the pairing code, pasted. The camera scan is a stub until a later task. */
export function PairScreen({ navigation, route }: Props) {
  const binding = useBinding();
  const palette = usePalette();
  const [mode, setMode] = useState<PairMode>(route.params?.mode ?? 'scan');
  const [code, setCode] = useState('');
  const [invalid, setInvalid] = useState(false);

  function changeCode(text: string) {
    setCode(text);
    setInvalid(false);
  }

  function connect() {
    const offer = code.trim();
    try {
      binding.parseOffer(offer);
    } catch {
      setInvalid(true);
      return;
    }
    navigation.navigate('ConfirmPairing', { offer });
  }

  return (
    <ScrollView
      style={{ backgroundColor: palette['surface.canvas'] }}
      contentContainerStyle={styles.content}
      // With the keyboard open, a tap on a button must press it, not only close the keyboard.
      keyboardShouldPersistTaps="handled"
    >
      <SegmentedControl
        accessibilityLabel="Pairing mode"
        options={[
          { value: 'scan', label: SCAN_TAB },
          { value: 'paste', label: PASTE_TAB },
        ]}
        value={mode}
        onChange={setMode}
      />
      {mode === 'scan' ? (
        <View style={styles.section}>
          <Text style={[styles.line, { color: palette['text.secondary'] }]}>{SCAN_LATER}</Text>
          <ActionButton label={PASTE_CODE_INSTEAD} variant="secondary" onPress={() => setMode('paste')} />
        </View>
      ) : (
        <View style={styles.section}>
          <TextField
            testID="pairing-code"
            label={PAIRING_CODE_LABEL}
            value={code}
            onChangeText={changeCode}
            multiline
            mono
            error={invalid ? INVALID_CODE : undefined}
          />
          <ActionButton label={CONNECT} variant="primary" disabled={code.trim() === '' || invalid} onPress={connect} />
        </View>
      )}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  content: {
    padding: spacingTokens['space.4'],
    gap: spacingTokens['space.4'],
  },
  section: {
    gap: spacingTokens['space.3'],
  },
  line: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
  },
});
