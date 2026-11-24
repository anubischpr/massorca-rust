import { StyleSheet, Text, View } from 'react-native';

import { typeTokens } from '../theme/tokens.generated';
import { usePalette } from '../theme/palette';

/** A screen whose real content comes in a later task: it shows its title and nothing else. */
export function PlaceholderScreen({ title }: { title: string }) {
  const palette = usePalette();
  return (
    <View style={[styles.container, { backgroundColor: palette['surface.canvas'] }]}>
      <Text style={[styles.title, { color: palette['text.primary'] }]}>{title}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 16,
  },
  title: {
    fontSize: typeTokens['a-heading'].size,
    lineHeight: typeTokens['a-heading'].lineHeight,
    textAlign: 'center',
  },
});
