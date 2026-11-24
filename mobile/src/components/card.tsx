import type { ReactNode } from 'react';
import { StyleSheet, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens } from '../theme/tokens.generated';

/** A grouped surface for facts and actions (screen spec, Card). */
export function Card({ children }: { readonly children: ReactNode }) {
  const palette = usePalette();
  return (
    <View style={[styles.card, { backgroundColor: palette['surface.raised'], borderColor: palette['border.default'] }]}>
      {children}
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    borderWidth: 1,
    borderRadius: radiusTokens['radius.lg'],
    padding: spacingTokens['space.3'],
    gap: spacingTokens['space.1'],
  },
});
