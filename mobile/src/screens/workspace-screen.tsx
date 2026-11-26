// A workspace's views (the session screen's view group, screen spec 7.1): Changes, read only, and its terminals.

import { useState } from 'react';
import { StyleSheet, View } from 'react-native';

import { SegmentedControl } from '../components/segmented-control';
import { CHANGES_VIEW } from '../copy/changes';
import { TERMINAL_TITLE } from '../copy/terminal';
import { WORKSPACE_VIEWS_LABEL } from '../copy/workspaces';
import { usePalette } from '../theme/palette';
import { spacingTokens } from '../theme/tokens.generated';
import { ChangesScreen } from './changes-screen';
import { TerminalListScreen } from './terminal-list-screen';

export interface WorkspaceScreenProps {
  readonly host: string;
  readonly workspace: string;
  /** A plain folder, not a git worktree: its Changes view says so. */
  readonly noGit: boolean;
  readonly onOpenDetails: () => void;
  readonly onOpenTerminal: (terminal: string) => void;
}

type WorkspaceView = 'changes' | 'terminal';

const VIEWS: readonly { readonly value: WorkspaceView; readonly label: string }[] = [
  { value: 'changes', label: CHANGES_VIEW },
  { value: 'terminal', label: TERMINAL_TITLE },
];

export function WorkspaceScreen({ host, workspace, noGit, onOpenDetails, onOpenTerminal }: WorkspaceScreenProps) {
  const palette = usePalette();
  const [view, setView] = useState<WorkspaceView>('changes');

  return (
    <View style={[styles.screen, { backgroundColor: palette['surface.canvas'] }]}>
      <View style={styles.views}>
        <SegmentedControl accessibilityLabel={WORKSPACE_VIEWS_LABEL} options={VIEWS} value={view} onChange={setView} />
      </View>
      {view === 'changes' ? (
        <ChangesScreen host={host} workspace={workspace} noGit={noGit} onOpenDetails={onOpenDetails} />
      ) : (
        <TerminalListScreen host={host} workspace={workspace} onOpenDetails={onOpenDetails} onOpenTerminal={onOpenTerminal} />
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
  },
  views: {
    paddingHorizontal: spacingTokens['space.4'],
    paddingTop: spacingTokens['space.2'],
  },
});
