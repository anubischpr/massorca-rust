// The Terminal view of a workspace (screen spec 9.1, the Terminal segment): the terminals the host lists for it. When it
// has exactly one live terminal, that one opens at once; otherwise the list shows, and tapping a row opens its terminal.
// Losing the link keeps the last list, with its age, the same way the Changes view does.

import { useEffect, useRef, useState, type ReactNode } from 'react';
import { FlatList, Pressable, StyleSheet, Text, View } from 'react-native';

import { FfiException, type TerminalSummary } from '../binding/types';
import { ConnectionBanner } from '../components/connection-banner';
import { LoadingRows } from '../components/loading-rows';
import { Overline } from '../components/overline';
import { StatePage } from '../components/state-page';
import { DETAILS, RETRY } from '../copy/connection';
import {
  LOADING_TERMINALS,
  NO_TERMINAL_LINE,
  NO_TERMINAL_TITLE,
  TERMINALS_FAILED_TITLE,
  TERMINALS_OVERLINE,
} from '../copy/terminal';
import { useHostRecord, useHostStore } from '../state/host-store-context';
import { INITIAL_LINK } from '../state/link-tracker';
import { useNow } from '../state/use-now';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens, type ColorTokenName } from '../theme/tokens.generated';
import { linkBannerModel } from './link-banner-model';
import { openDirectlyTarget, terminalRows, type TerminalRow, type TerminalState } from './terminal-list-model';

export interface TerminalListScreenProps {
  readonly host: string;
  readonly workspace: string;
  readonly onOpenDetails: () => void;
  readonly onOpenTerminal: (terminal: string) => void;
}

type ReadState =
  | { readonly tag: 'loading' }
  | { readonly tag: 'ready'; readonly terminals: readonly TerminalSummary[] }
  | { readonly tag: 'failed'; readonly message: string };

export function TerminalListScreen({ host, workspace, onOpenDetails, onOpenTerminal }: TerminalListScreenProps) {
  const store = useHostStore();
  const palette = usePalette();
  const record = useHostRecord(host);
  const now = useNow(1_000);
  const [read, setRead] = useState<ReadState>({ tag: 'loading' });
  const [attempt, setAttempt] = useState(0);
  const opened = useRef(false);
  const connection = record?.connection;
  const connected = connection?.status === 'connected' ? connection.host : undefined;
  const agents = record?.agents;
  const restoredAtMs = agents?.restoredAtMs;
  const linkLost = agents?.link.tag === 'Unverifiable';

  useEffect(() => {
    store.ensure(host);
  }, [host, store]);

  useEffect(() => {
    if (connected === undefined) return undefined;
    let current = true;
    setRead({ tag: 'loading' });
    connected.terminals(workspace).then(
      (terminals) => {
        if (current) setRead({ tag: 'ready', terminals });
      },
      (error: unknown) => {
        if (current) setRead({ tag: 'failed', message: messageOf(error) });
      },
    );
    return () => {
      current = false;
    };
  }, [connected, workspace, attempt]);

  // The link came back: read the list again, and keep the last one while the new one is on its way.
  useEffect(() => {
    if (connected === undefined || restoredAtMs === undefined) return undefined;
    let current = true;
    connected.terminals(workspace).then(
      (terminals) => {
        if (current) setRead({ tag: 'ready', terminals });
      },
      (error: unknown) => {
        if (current) setRead((previous) => (previous.tag === 'ready' ? previous : { tag: 'failed', message: messageOf(error) }));
      },
    );
    return () => {
      current = false;
    };
  }, [connected, workspace, restoredAtMs]);

  const target = read.tag === 'ready' ? openDirectlyTarget(read.terminals) : undefined;
  useEffect(() => {
    if (target === undefined || opened.current) return;
    opened.current = true;
    onOpenTerminal(target);
  }, [target, onOpenTerminal]);

  const banner = linkBannerModel(host, agents ?? INITIAL_LINK, now);
  let body: ReactNode;
  if (connection === undefined || connection.status === 'connecting') {
    body = <LoadingRows label={LOADING_TERMINALS} />;
  } else if (connection.status === 'failed') {
    body = (
      <StatePage
        title={TERMINALS_FAILED_TITLE}
        detail={connection.error.message}
        actions={[{ label: RETRY, onPress: () => store.retry(host), variant: 'primary' }]}
      />
    );
  } else if (read.tag === 'loading') {
    body = <LoadingRows label={LOADING_TERMINALS} />;
  } else if (read.tag === 'failed') {
    body = (
      <StatePage
        title={TERMINALS_FAILED_TITLE}
        detail={read.message}
        actions={[{ label: RETRY, onPress: () => setAttempt((count) => count + 1), variant: 'primary' }]}
      />
    );
  } else if (read.terminals.length === 0) {
    body = <StatePage title={NO_TERMINAL_TITLE} line={NO_TERMINAL_LINE} />;
  } else {
    body = <TerminalsList rows={terminalRows(read.terminals)} dimmed={linkLost} onOpen={onOpenTerminal} />;
  }

  return (
    <View style={[styles.screen, { backgroundColor: palette['surface.canvas'] }]}>
      {banner !== undefined && (
        <View style={styles.banner}>
          <ConnectionBanner
            tone={banner.tone}
            message={banner.message}
            actions={banner.actions.length > 0 ? [{ label: DETAILS, onPress: onOpenDetails }] : []}
          />
        </View>
      )}
      {body}
    </View>
  );
}

function TerminalsList({
  rows,
  dimmed,
  onOpen,
}: {
  readonly rows: readonly TerminalRow[];
  readonly dimmed: boolean;
  readonly onOpen: (terminal: string) => void;
}) {
  return (
    <FlatList
      contentContainerStyle={styles.list}
      data={rows}
      keyExtractor={(row) => row.id}
      ListHeaderComponent={
        <View style={styles.header}>
          <Overline>{TERMINALS_OVERLINE(rows.length)}</Overline>
        </View>
      }
      renderItem={({ item }) => <TerminalRowView row={item} dimmed={dimmed} onOpen={() => onOpen(item.id)} />}
    />
  );
}

function TerminalRowView({ row, dimmed, onOpen }: { readonly row: TerminalRow; readonly dimmed: boolean; readonly onOpen: () => void }) {
  const palette = usePalette();
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={`${row.name}, ${row.word}, ${row.cwd}`}
      onPress={onOpen}
      style={[styles.row, dimmed && styles.dimmed]}
    >
      <View style={styles.names}>
        <Text numberOfLines={1} style={[styles.name, { color: palette['text.primary'] }]}>
          {row.name}
        </Text>
        <Text numberOfLines={1} style={[styles.cwd, { color: palette['text.tertiary'] }]}>
          {row.cwd}
        </Text>
      </View>
      <Text style={[styles.word, { color: palette[STATE_COLOUR[row.state]] }]}>{row.word}</Text>
    </Pressable>
  );
}

/** The colour of each state word; the word carries the meaning. */
const STATE_COLOUR: Record<TerminalState, ColorTokenName> = {
  live: 'status.success.fg',
  unverifiable: 'status.warning.fg',
  exited: 'text.secondary',
  unknown: 'text.tertiary',
};

function messageOf(error: unknown): string {
  return error instanceof FfiException ? error.error.message : String(error);
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
  },
  banner: {
    padding: spacingTokens['space.4'],
  },
  list: {
    paddingHorizontal: spacingTokens['space.4'],
    paddingBottom: spacingTokens['space.4'],
    gap: spacingTokens['space.2'],
  },
  header: {
    paddingTop: spacingTokens['space.2'],
  },
  row: {
    minHeight: 56,
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacingTokens['space.3'],
  },
  dimmed: {
    opacity: 0.6,
  },
  names: {
    flex: 1,
  },
  name: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
  },
  cwd: {
    fontFamily: 'monospace',
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
  },
  word: {
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '600',
  },
});
