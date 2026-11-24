import { StyleSheet, Text, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens, type ColorTokenName } from '../theme/tokens.generated';

export type BadgeTone = 'neutral' | 'info' | 'success' | 'warning' | 'danger' | 'unreachable';

const TONE: Record<BadgeTone, { readonly bg: ColorTokenName; readonly fg: ColorTokenName; readonly border: ColorTokenName }> =
  {
    neutral: { bg: 'badge.neutral.bg', fg: 'badge.neutral.fg', border: 'border.subtle' },
    info: { bg: 'status.info.bg', fg: 'status.info.fg', border: 'status.info.border' },
    success: { bg: 'status.success.bg', fg: 'status.success.fg', border: 'status.success.border' },
    warning: { bg: 'status.warning.bg', fg: 'status.warning.fg', border: 'status.warning.border' },
    danger: { bg: 'status.danger.bg', fg: 'status.danger.fg', border: 'status.danger.border' },
    unreachable: { bg: 'agent.unreachable.bg', fg: 'text.tertiary', border: 'agent.unreachable.border' },
  };

/** A short state label; the words carry the meaning, the tone only adds to it. */
export function StateBadge({ label, tone }: { readonly label: string; readonly tone: BadgeTone }) {
  const palette = usePalette();
  const colours = TONE[tone];
  return (
    <View
      accessibilityLabel={label}
      style={[
        styles.badge,
        { backgroundColor: palette[colours.bg], borderColor: palette[colours.border] },
      ]}
    >
      <Text style={[styles.label, { color: palette[colours.fg] }]}>{label}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  badge: {
    alignSelf: 'flex-start',
    borderWidth: 1,
    borderRadius: radiusTokens['radius.sm'],
    paddingHorizontal: spacingTokens['space.2'],
    paddingVertical: spacingTokens['space.half'],
  },
  label: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
    fontWeight: '600',
  },
});
