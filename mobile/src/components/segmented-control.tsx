import { Pressable, StyleSheet, Text, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens } from '../theme/tokens.generated';
import { CONTROL_HEIGHT } from './dimensions';

/** Two or more exclusive options, each a 44 dp segment (screen spec 4.3). */
export function SegmentedControl<T extends string>({
  options,
  value,
  onChange,
  accessibilityLabel,
}: {
  readonly options: readonly { readonly value: T; readonly label: string }[];
  readonly value: T;
  readonly onChange: (next: T) => void;
  readonly accessibilityLabel: string;
}) {
  const palette = usePalette();
  return (
    <View accessibilityRole="tablist" accessibilityLabel={accessibilityLabel} style={[styles.track, { backgroundColor: palette['surface.sunken'] }]}>
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <Pressable
            key={option.value}
            accessibilityRole="tab"
            accessibilityLabel={option.label}
            accessibilityState={{ selected }}
            onPress={() => onChange(option.value)}
            style={[styles.segment, selected && { backgroundColor: palette['fill.selected.raised'] }]}
          >
            <Text style={[styles.label, { color: selected ? palette['text.primary'] : palette['text.secondary'] }]}>
              {option.label}
            </Text>
          </Pressable>
        );
      })}
    </View>
  );
}

const styles = StyleSheet.create({
  track: {
    flexDirection: 'row',
    borderRadius: radiusTokens['radius.lg'],
    padding: spacingTokens['space.half'],
    gap: spacingTokens['space.half'],
  },
  segment: {
    flex: 1,
    minHeight: CONTROL_HEIGHT,
    borderRadius: radiusTokens['radius.md'],
    alignItems: 'center',
    justifyContent: 'center',
  },
  label: {
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '500',
  },
});
