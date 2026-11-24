import { StyleSheet, Text } from 'react-native';

import { usePalette } from '../theme/palette';
import { typeTokens } from '../theme/tokens.generated';

/** A section label in capitals (the overline of screen spec 5.1); it is a heading for screen readers. */
export function Overline({ children }: { readonly children: string }) {
  const palette = usePalette();
  return (
    <Text accessibilityRole="header" style={[styles.overline, { color: palette['text.secondary'] }]}>
      {children}
    </Text>
  );
}

const styles = StyleSheet.create({
  overline: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
    fontWeight: '600',
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
});
