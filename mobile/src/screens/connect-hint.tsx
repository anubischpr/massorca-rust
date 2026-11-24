import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { ActionButton } from '../components/action-button';
import { CONTROL_HEIGHT } from '../components/dimensions';
import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens } from '../theme/tokens.generated';
import { CONNECT_STEPS, HOME_EMPTY_LINE, HOME_EMPTY_TITLE, PAIR_DESKTOP, PASTE_A_CODE } from '../copy/hosts';

/** M-PA-01: how to connect a desktop, shown on an empty Home. */
export function ConnectHint({ onPair, onPaste }: { readonly onPair: () => void; readonly onPaste: () => void }) {
  const palette = usePalette();
  return (
    <View style={[styles.screen, { backgroundColor: palette['surface.canvas'] }]}>
      <ScrollView contentContainerStyle={styles.content}>
        <Text accessibilityRole="header" style={[styles.title, { color: palette['text.primary'] }]}>
          {HOME_EMPTY_TITLE}
        </Text>
        <Text style={[styles.line, { color: palette['text.secondary'] }]}>{HOME_EMPTY_LINE}</Text>
        {CONNECT_STEPS.map((step, index) => (
          <View key={step.title} style={styles.step}>
            <View style={[styles.number, { backgroundColor: palette['accent.subtle.canvas'] }]}>
              <Text style={[styles.numberText, { color: palette['accent.fg'] }]}>{index + 1}</Text>
            </View>
            <View style={styles.stepText}>
              <Text style={[styles.stepTitle, { color: palette['text.primary'] }]}>{step.title}</Text>
              <Text style={[styles.stepLine, { color: palette['text.secondary'] }]}>{step.line}</Text>
            </View>
          </View>
        ))}
      </ScrollView>
      <View style={styles.pinned}>
        <ActionButton label={PAIR_DESKTOP} variant="primary" onPress={onPair} />
        <ActionButton label={PASTE_A_CODE} variant="ghost" onPress={onPaste} />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
    padding: spacingTokens['space.4'],
  },
  content: {
    gap: spacingTokens['space.4'],
    paddingBottom: spacingTokens['space.4'],
  },
  title: {
    fontSize: typeTokens['a-display'].size,
    lineHeight: typeTokens['a-display'].lineHeight,
    fontWeight: '600',
  },
  line: {
    fontSize: typeTokens['a-chat'].size,
    lineHeight: typeTokens['a-chat'].lineHeight,
  },
  step: {
    flexDirection: 'row',
    gap: spacingTokens['space.3'],
  },
  number: {
    width: 28,
    height: 28,
    borderRadius: radiusTokens['radius.full'],
    alignItems: 'center',
    justifyContent: 'center',
  },
  numberText: {
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '600',
  },
  stepText: {
    flex: 1,
    gap: spacingTokens['space.half'],
    minHeight: CONTROL_HEIGHT,
  },
  stepTitle: {
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '600',
  },
  stepLine: {
    fontSize: typeTokens['a-body-sm'].size,
    lineHeight: typeTokens['a-body-sm'].lineHeight,
  },
  pinned: {
    gap: spacingTokens['space.2'],
    paddingTop: spacingTokens['space.2'],
  },
});
