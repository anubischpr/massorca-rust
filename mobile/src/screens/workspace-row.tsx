import { Pressable, StyleSheet, Text, View } from 'react-native';

import { AgentStatusGlyph } from '../components/agent-status-glyph';
import { GLYPH_SIZE, TOUCH_TARGET } from '../components/dimensions';
import { StateBadge } from '../components/state-badge';
import { NO_GIT_ACTION } from '../copy/folder-without-git';
import { NO_GIT_BADGE } from '../copy/workspaces';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';
import type { WorkspaceRow } from './workspace-sections';

/**
 * One workspace row (M-WS-01): glyph, name, No Git, then the state word. Without contact the word is the last known state.
 * Tapping the row opens the workspace's views; tapping the No Git badge opens the folder's sheet (M-WS-05), which TalkBack
 * also offers as a custom action on the row.
 */
export function WorkspaceRowView({
  row,
  onOpen,
  onNoGit,
}: {
  readonly row: WorkspaceRow;
  readonly onOpen: () => void;
  readonly onNoGit?: () => void;
}) {
  const palette = usePalette();
  const dimmed = row.agent?.dimmed === true;
  const label = [row.name, row.agent?.word, row.noGit ? NO_GIT_BADGE : undefined].filter(Boolean).join(', ');
  const noGitAction = row.noGit && onNoGit !== undefined ? [{ name: 'noGit', label: NO_GIT_ACTION }] : undefined;

  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={label}
      accessibilityActions={noGitAction}
      onAccessibilityAction={(event) => {
        if (event.nativeEvent.actionName === 'noGit') onNoGit?.();
      }}
      onPress={onOpen}
      style={styles.row}
    >
      <View style={styles.slot}>
        {row.agent !== undefined && (
          <AgentStatusGlyph state={row.agent.glyph} word={row.agent.word} size={GLYPH_SIZE.large} />
        )}
      </View>
      <Text numberOfLines={1} style={[styles.name, { color: dimmed ? palette['text.tertiary'] : palette['text.primary'] }]}>
        {row.name}
      </Text>
      {row.noGit && (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={NO_GIT_BADGE}
          disabled={onNoGit === undefined}
          onPress={onNoGit}
          style={styles.badgeTarget}
          testID="no-git-badge"
        >
          <StateBadge label={NO_GIT_BADGE} tone="neutral" />
        </Pressable>
      )}
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
  badgeTarget: {
    minHeight: TOUCH_TARGET,
    justifyContent: 'center',
  },
  word: {
    flexShrink: 1,
    marginLeft: 'auto',
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
    fontVariant: ['tabular-nums'],
  },
});
