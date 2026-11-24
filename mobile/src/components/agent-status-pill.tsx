import { StyleSheet, Text, View } from 'react-native';

import { usePalette } from '../theme/palette';
import { radiusTokens, spacingTokens, typeTokens } from '../theme/tokens.generated';
import { AgentStatusGlyph, type GlyphState } from './agent-status-glyph';
import { GLYPH_SIZE, PILL_HEIGHT } from './dimensions';

/** The status pill of a session (M-SE-07): the agent's glyph and its word, with the last known state when contact is lost. */
export function AgentStatusPill({ glyph, word }: { readonly glyph: GlyphState; readonly word: string }) {
  const palette = usePalette();
  return (
    <View
      accessible
      accessibilityLabel={word}
      style={[styles.pill, { backgroundColor: palette['surface.raised'], borderColor: palette['border.default'] }]}
    >
      <AgentStatusGlyph state={glyph} word={word} size={GLYPH_SIZE.medium} />
      <Text numberOfLines={1} style={[styles.word, { color: palette['text.primary'] }]}>
        {word}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  pill: {
    minHeight: PILL_HEIGHT,
    flexDirection: 'row',
    alignItems: 'center',
    alignSelf: 'flex-start',
    gap: spacingTokens['space.2'],
    borderWidth: 1,
    borderRadius: radiusTokens['radius.full'],
    paddingHorizontal: spacingTokens['space.3'],
  },
  word: {
    flexShrink: 1,
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '500',
  },
});
