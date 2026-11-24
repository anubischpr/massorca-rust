import { useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { useBinding } from '../binding/binding-context';
import { usePalette } from '../theme/palette';
import { typeTokens } from '../theme/tokens.generated';
import { PlaceholderScreen } from './placeholder-screen';

/**
 * Home (M-HM-01). With no host paired it shows the connect hint of M-PA-01; the host list comes in a later task.
 */
export function HomeScreen() {
  const binding = useBinding();
  const [hostCount] = useState(() => binding.hosts().length);
  const palette = usePalette();

  if (hostCount > 0) return <PlaceholderScreen title="Home" />;

  return (
    <View style={[styles.container, { backgroundColor: palette['surface.canvas'] }]}>
      <Text style={[styles.title, { color: palette['text.primary'] }]}>Connect your desktop</Text>
      <Text style={[styles.line, { color: palette['text.secondary'] }]}>
        Follow your agents from the phone, answer their questions and approve their work.
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    justifyContent: 'center',
    padding: 16,
    gap: 12,
  },
  title: {
    fontSize: typeTokens['a-display'].size,
    lineHeight: typeTokens['a-display'].lineHeight,
  },
  line: {
    fontSize: typeTokens['a-chat'].size,
    lineHeight: typeTokens['a-chat'].lineHeight,
  },
});
