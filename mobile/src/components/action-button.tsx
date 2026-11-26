import { ActivityIndicator, Pressable, StyleSheet, Text } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens, type ColorTokenName } from '../theme/tokens.generated';
import { TOUCH_TARGET } from './dimensions';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

const VARIANT: Record<
  ButtonVariant,
  { readonly bg: ColorTokenName | 'transparent'; readonly text: ColorTokenName; readonly border: ColorTokenName | undefined }
> = {
  primary: { bg: 'accent.default', text: 'text.on-accent', border: undefined },
  secondary: { bg: 'surface.raised', text: 'text.primary', border: 'border.default' },
  ghost: { bg: 'transparent', text: 'accent.fg', border: undefined },
  danger: { bg: 'surface.raised', text: 'status.danger.fg', border: 'status.danger.border' },
};

/** A control with a 48 dp hit target. `busy` keeps the label and shows a spinner in place of nothing. */
export function ActionButton({
  label,
  onPress,
  variant = 'secondary',
  disabled = false,
  busy = false,
  testID,
}: {
  readonly label: string;
  readonly onPress: () => void;
  readonly variant?: ButtonVariant;
  readonly disabled?: boolean;
  readonly busy?: boolean;
  readonly testID?: string;
}) {
  const palette = usePalette();
  const style = VARIANT[variant];
  const inactive = disabled || busy;
  const textColour = inactive ? palette['text.disabled'] : palette[style.text];
  const background = style.bg === 'transparent' ? 'transparent' : palette[style.bg];

  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityState={{ disabled: inactive, busy }}
      disabled={inactive}
      onPress={onPress}
      testID={testID}
      style={({ pressed }) => [
        styles.button,
        {
          backgroundColor: inactive && variant === 'primary' ? palette['fill.disabled'] : background,
          borderColor: style.border === undefined ? 'transparent' : palette[style.border],
          opacity: pressed ? 0.85 : 1,
        },
      ]}
    >
      {busy && <ActivityIndicator color={textColour} style={styles.spinner} />}
      <Text style={[styles.label, { color: textColour }]}>{label}</Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  button: {
    minHeight: TOUCH_TARGET,
    minWidth: TOUCH_TARGET,
    borderWidth: 1,
    borderRadius: radiusTokens['radius.lg'],
    paddingHorizontal: spacingTokens['space.4'],
    alignItems: 'center',
    justifyContent: 'center',
    flexDirection: 'row',
  },
  spinner: {
    marginRight: spacingTokens['space.2'],
  },
  label: {
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '500',
    textAlign: 'center',
  },
});
