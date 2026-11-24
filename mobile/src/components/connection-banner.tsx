import { StyleSheet, Text, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens, type ColorTokenName } from '../theme/tokens.generated';
import { ActionButton } from './action-button';

export type BannerTone = 'info' | 'warning' | 'danger' | 'success' | 'unreachable';

const TONE: Record<BannerTone, { readonly bg: ColorTokenName; readonly border: ColorTokenName; readonly text: ColorTokenName }> = {
  info: { bg: 'status.info.bg', border: 'status.info.border', text: 'status.info.fg' },
  warning: { bg: 'status.warning.bg', border: 'status.warning.border', text: 'text.primary' },
  danger: { bg: 'status.danger.bg', border: 'status.danger.border', text: 'text.primary' },
  success: { bg: 'status.success.bg', border: 'status.success.border', text: 'text.primary' },
  unreachable: { bg: 'agent.unreachable.bg', border: 'agent.unreachable.border', text: 'text.primary' },
};

export interface BannerAction {
  readonly label: string;
  readonly onPress: () => void;
}

/** X-CN-01: a full-width notice under the top bar. Announced politely, never as an overlay. */
export function ConnectionBanner({
  tone,
  message,
  actions = [],
}: {
  readonly tone: BannerTone;
  readonly message: string;
  readonly actions?: readonly BannerAction[];
}) {
  const palette = usePalette();
  const colours = TONE[tone];
  return (
    <View
      accessibilityLiveRegion="polite"
      style={[styles.banner, { backgroundColor: palette[colours.bg], borderColor: palette[colours.border] }]}
    >
      <Text style={[styles.message, { color: palette[colours.text] }]}>{message}</Text>
      {actions.length > 0 && (
        <View style={styles.actions}>
          {actions.map((action) => (
            <ActionButton key={action.label} label={action.label} onPress={action.onPress} variant="ghost" />
          ))}
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  banner: {
    borderWidth: 1,
    borderRadius: radiusTokens['radius.lg'],
    padding: spacingTokens['space.3'],
    gap: spacingTokens['space.2'],
  },
  message: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
  },
  actions: {
    flexDirection: 'row',
    gap: spacingTokens['space.2'],
  },
});
