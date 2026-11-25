import { Pressable, StyleSheet, Text, View } from 'react-native';

import { AgentStatusGlyph } from '../components/agent-status-glyph';
import { GLYPH_SIZE, TOUCH_TARGET } from '../components/dimensions';
import { StateBadge } from '../components/state-badge';
import { NO_GIT_BADGE } from '../copy/workspaces';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';
import type { WorkspaceRow } from './workspace-sections';

/**
 * One workspace row (M-WS-01): glyph, name, No Git, then the state word. Without contact the word is the last known state.
 * Tapping it opens the workspace's views: its source control and its terminals.
 */
export function WorkspaceRowView({ row, onOpen }: { readonly row: WorkspaceRow; readonly onOpen: () => void }) {
  const palette = usePalette();
  const dimmed = row.agent?.dimmed === true;
  const label = [row.name, row.agent?.word, row.noGit ? NO_GIT_BADGE : undefined].filter(Boolean).join(', ');

  return (
    <Pressable accessibilityRole="button" accessibilityLabel={label} onPress={onOpen} style={styles.row}>
      <View style={styles.slot}>
        {row.agent !== undefined && (
          <AgentStatusGlyph state={row.agent.glyph} word={row.agent.word} size={GLYPH_SIZE.large} />
        )}
      </View>
      <Text numberOfLines={1} style={[styles.name, { color: dimmed ? palette['text.tertiary'] : palette['text.primary'] }]}>
        {row.name}
      </Text>
      {row.noGit && <StateBadge label={NO_GIT_BADGE} tone="neutral" />}
      {row.agent !== undefined && (
        <Text numberOfLines={1} style={[styles.word, { color: palette['text.secondary'] }]}>
          {row.agent.word}
        </Text>
      )}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  row: {
    minHeight: TOUCH_TARGET,
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacingTokens['space.3'],
    paddingHorizontal: spacingTokens['space.4'],
  },
  slot: {
    width: 24,
    height: 24,
    alignItems: 'center',
    justifyContent: 'center',
  },
  name: {
    flexShrink: 1,
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
  },
  word: {
    flexShrink: 1,
    marginLeft: 'auto',
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
    fontVariant: ['tabular-nums'],
  },
});
