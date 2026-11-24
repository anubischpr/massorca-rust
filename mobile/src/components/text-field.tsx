import { useState } from 'react';
import { StyleSheet, Text, TextInput, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens } from '../theme/tokens.generated';
import { MULTILINE_FIELD_HEIGHT, TOUCH_TARGET } from './dimensions';

/** A labelled field. `error` replaces the helper line and turns the border to the danger colour. */
export function TextField({
  label,
  value,
  onChangeText,
  multiline = false,
  mono = false,
  helper,
  error,
  editable = true,
}: {
  readonly label: string;
  readonly value: string;
  readonly onChangeText: (text: string) => void;
  readonly multiline?: boolean;
  readonly mono?: boolean;
  readonly helper?: string;
  readonly error?: string;
  readonly editable?: boolean;
}) {
  const palette = usePalette();
  const [focused, setFocused] = useState(false);
  const borderColour = error !== undefined ? palette['status.danger.fg'] : focused ? palette['border.focus'] : palette['border.default'];
  const typography = mono ? typeTokens['a-mono'] : typeTokens['a-body'];
  const fontFamily = mono ? 'monospace' : undefined;

  return (
    <View style={styles.wrap}>
      <Text style={[styles.label, { color: palette['text.secondary'] }]}>{label}</Text>
      <TextInput
        accessibilityLabel={label}
        value={value}
        onChangeText={onChangeText}
        editable={editable}
        multiline={multiline}
        textAlignVertical={multiline ? 'top' : 'center'}
        autoCapitalize="none"
        autoCorrect={false}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        placeholderTextColor={palette['text.tertiary']}
        style={[
          styles.input,
          multiline ? styles.multiline : styles.single,
          {
            color: palette['text.primary'],
            backgroundColor: palette['surface.sunken'],
            borderColor: borderColour,
            fontFamily,
            fontSize: typography.size,
            lineHeight: typography.lineHeight,
          },
        ]}
      />
      {error !== undefined ? (
        <Text style={[styles.note, { color: palette['status.danger.fg'] }]}>{error}</Text>
      ) : helper !== undefined ? (
        <Text style={[styles.note, { color: palette['text.secondary'] }]}>{helper}</Text>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  wrap: {
    gap: spacingTokens['space.1'],
  },
  label: {
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '500',
  },
  input: {
    borderWidth: 1,
    borderRadius: radiusTokens['radius.lg'],
    paddingHorizontal: spacingTokens['space.3'],
    paddingVertical: spacingTokens['space.2'],
  },
  single: {
    minHeight: TOUCH_TARGET,
  },
  multiline: {
    height: MULTILINE_FIELD_HEIGHT,
  },
  note: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
  },
});
