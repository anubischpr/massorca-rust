import { StyleSheet, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens } from '../theme/tokens.generated';
import { TOUCH_TARGET } from './dimensions';

const BAR_WIDTHS = ['62%', '48%', '74%', '55%'] as const;

/** Placeholder rows shown while a list loads; they carry the loading label for screen readers. */
export function LoadingRows({ label }: { readonly label: string }) {
  const palette = usePalette();
  return (
    <View accessibilityLabel={label} accessibilityRole="progressbar" style={styles.list}>
      {BAR_WIDTHS.map((width) => (
        <View key={width} style={styles.row}>
          <View style={[styles.dot, { backgroundColor: palette['fill.skeleton'] }]} />
          <View style={[styles.bar, { width, backgroundColor: palette['fill.skeleton'] }]} />
        </View>
      ))}
    </View>
  );
}

const styles = StyleSheet.create({
  list: {
    paddingHorizontal: spacingTokens['space.4'],
    gap: spacingTokens['space.2'],
  },
  row: {
    minHeight: TOUCH_TARGET,
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacingTokens['space.3'],
  },
  dot: {
    width: 24,
    height: 24,
    borderRadius: 12,
  },
  bar: {
    height: 14,
    borderRadius: radiusTokens['radius.sm'],
  },
});
