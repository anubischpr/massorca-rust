// Source control, the Changes segment (M-DF-02), read only in M5: the branch and the changed files of one workspace, read
// now and again whenever the host reports a change to its git state. Losing the link keeps the last list, with its age.

import { useEffect, useState, type ReactNode } from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';

import { FfiException, type GitEvent, type GitStatusSummary, type Watch } from '../binding/types';
import { Card } from '../components/card';
import { ConnectionBanner } from '../components/connection-banner';
import { LoadingRows } from '../components/loading-rows';
import { Overline } from '../components/overline';
import { StateBadge } from '../components/state-badge';
import { StatePage } from '../components/state-page';
import { DETAILS, RETRY } from '../copy/connection';
import {
  CHANGE_KIND_WORDS,
  CHANGES_FAILED_TITLE,
  CHANGES_OVERLINE,
  CONFLICTS_LINE,
  DETACHED_HEAD,
  LOADING_CHANGES,
  NO_CHANGES_LINE,
  NO_CHANGES_TITLE,
  NO_GIT_TITLE,
  TRUNCATED_LINE,
} from '../copy/changes';
import { useHostRecord, useHostStore } from '../state/host-store-context';
import { INITIAL_LINK, touchContact, trackLink, type LinkTracker } from '../state/link-tracker';
import { useNow } from '../state/use-now';
import { usePalette, type Palette } from '../theme/palette';
import { spacingTokens, typeTokens, type ColorTokenName } from '../theme/tokens.generated';
import { aheadBehindLine, changeRows, conflictCount, type ChangeKind, type ChangeRow } from './changes-model';
import { linkBannerModel } from './link-banner-model';

export interface ChangesScreenProps {
  readonly host: string;
  readonly workspace: string;
  /** A plain folder, not a git worktree: there is no status to read. */
  readonly noGit: boolean;
  readonly onOpenDetails: () => void;
}

type ReadState =
  | { readonly tag: 'loading' }
  | { readonly tag: 'ready'; readonly status: GitStatusSummary }
  | { readonly tag: 'failed'; readonly message: string };

export function ChangesScreen({ host, workspace, noGit, onOpenDetails }: ChangesScreenProps) {
  const store = useHostStore();
  const palette = usePalette();
  const record = useHostRecord(host);
  const now = useNow(1_000);
  const [read, setRead] = useState<ReadState>({ tag: 'loading' });
  const [tracker, setTracker] = useState<LinkTracker>(INITIAL_LINK);
  const [attempt, setAttempt] = useState(0);
  const connection = record?.connection;
  const connected = connection?.status === 'connected' ? connection.host : undefined;

  useEffect(() => {
    store.ensure(host);
  }, [host, store]);

  useEffect(() => {
    if (connected === undefined || noGit) return undefined;
    let current = true;
    setRead({ tag: 'loading' });
    setTracker(INITIAL_LINK);

    const readStatus = (): void => {
      connected.gitStatus(workspace).then(
        (status) => {
          if (current) setRead({ tag: 'ready', status });
        },
        (error: unknown) => {
          // A read that fails after a list was shown keeps that list: the banner says the link is lost.
          if (current) setRead((previous) => (previous.tag === 'ready' ? previous : { tag: 'failed', message: messageOf(error) }));
        },
      );
    };

    const stream: Watch<GitEvent> = connected.watchGit(workspace);
    const follow = async (): Promise<void> => {
      try {
        for (let event = await stream.next(); event !== null && current; event = await stream.next()) {
          const at = Date.now();
          if (event.tag === 'Item') {
            setTracker((previous) => touchContact(previous, at));
            readStatus();
          } else {
            const { link } = event;
            setTracker((previous) => trackLink(previous, link, at));
            if (link.tag === 'Live') readStatus();
          }
        }
      } catch (error) {
        const reason = messageOf(error);
        if (current) {
          setTracker((previous) =>
            trackLink(previous, { tag: 'Unverifiable', attempt: 1, retryInMs: 0n, reason }, Date.now()),
          );
        }
      }
    };

    readStatus();
    follow();
    return () => {
      current = false;
      stream.close();
    };
  }, [connected, workspace, noGit, attempt, store]);

  function retry(): void {
    setAttempt((count) => count + 1);
  }

  const banner = linkBannerModel(host, tracker, now);
  let body: ReactNode;
  if (noGit) {
    body = <StatePage title={NO_GIT_TITLE} />;
  } else if (connection?.status === 'failed') {
    body = (
      <StatePage
        title={CHANGES_FAILED_TITLE}
        detail={connection.error.message}
        actions={[{ label: RETRY, onPress: () => store.retry(host), variant: 'primary' }]}
      />
    );
  } else if (read.tag === 'loading') {
    body = <LoadingRows label={LOADING_CHANGES} />;
  } else if (read.tag === 'failed') {
    body = (
      <StatePage
        title={CHANGES_FAILED_TITLE}
        detail={read.message}
        actions={[{ label: RETRY, onPress: retry, variant: 'primary' }]}
      />
    );
  } else {
    body = <ChangesList status={read.status} host={host} dimmed={tracker.link.tag === 'Unverifiable'} />;
  }

  return (
    <View style={[styles.screen, { backgroundColor: palette['surface.canvas'] }]}>
      {!noGit && banner !== undefined && (
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

function ChangesList({
  status,
  host,
  dimmed,
}: {
  readonly status: GitStatusSummary;
  readonly host: string;
  readonly dimmed: boolean;
}) {
  const palette = usePalette();
  const rows = changeRows(status.files);
  const conflicts = conflictCount(status.files);
  return (
    <FlatList
      contentContainerStyle={styles.list}
      data={rows}
      keyExtractor={(row) => row.path}
      ListEmptyComponent={
        <View style={styles.empty}>
          <Text style={[styles.emptyTitle, { color: palette['text.primary'] }]}>{NO_CHANGES_TITLE}</Text>
          <Text style={[styles.emptyLine, { color: palette['text.secondary'] }]}>{NO_CHANGES_LINE}</Text>
        </View>
      }
      ListFooterComponent={
        status.truncated ? (
          <Text style={[styles.caption, { color: palette['text.tertiary'] }]}>{TRUNCATED_LINE}</Text>
        ) : undefined
      }
      ListHeaderComponent={
        <View style={styles.header}>
          <BranchCard status={status} />
          {conflicts > 0 && (
            <ConnectionBanner tone="danger" message={CONFLICTS_LINE(conflicts, host)} actions={[]} />
          )}
          {rows.length > 0 && <Overline>{CHANGES_OVERLINE(rows.length)}</Overline>}
        </View>
      }
      renderItem={({ item }) => <ChangeRowView row={item} palette={palette} dimmed={dimmed} />}
    />
  );
}

function BranchCard({ status }: { readonly status: GitStatusSummary }) {
  const palette = usePalette();
  const detached = status.branch === undefined;
  const ahead = aheadBehindLine(status);
  return (
    <Card>
      {detached ? (
        <StateBadge label={DETACHED_HEAD} tone="warning" />
      ) : (
        <Text numberOfLines={1} style={[styles.branch, { color: palette['text.primary'] }]}>
          {status.branch}
        </Text>
      )}
      {ahead !== undefined && <Text style={[styles.caption, { color: palette['text.secondary'] }]}>{ahead}</Text>}
    </Card>
  );
}

function ChangeRowView({ row, palette, dimmed }: { readonly row: ChangeRow; readonly palette: Palette; readonly dimmed: boolean }) {
  const words = CHANGE_KIND_WORDS[row.kind];
  return (
    <View accessible accessibilityLabel={`${words.word}, ${row.path}`} style={[styles.row, dimmed && styles.dimmed]}>
      <Text style={[styles.letter, { color: palette[KIND_COLOUR[row.kind]] }]}>{words.letter}</Text>
      <View style={styles.names}>
        <Text numberOfLines={1} style={[styles.name, { color: palette['text.primary'] }]}>
          {row.name}
        </Text>
        {row.directory !== undefined && (
          <Text numberOfLines={1} style={[styles.directory, { color: palette['text.tertiary'] }]}>
            {row.directory}
          </Text>
        )}
      </View>
    </View>
  );
}

/** The colour of each letter; the word beside it is what carries the meaning. */
const KIND_COLOUR: Record<ChangeKind, ColorTokenName> = {
  conflict: 'status.danger.fg',
  added: 'diff.add.fg',
  untracked: 'diff.add.fg',
  modified: 'status.warning.fg',
  deleted: 'diff.remove.fg',
  renamed: 'status.info.fg',
  copied: 'status.info.fg',
  typeChanged: 'status.warning.fg',
  unknown: 'text.secondary',
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
    gap: spacingTokens['space.2'],
    paddingTop: spacingTokens['space.2'],
  },
  branch: {
    fontFamily: 'monospace',
    fontSize: typeTokens['a-mono'].size,
    lineHeight: typeTokens['a-mono'].lineHeight,
  },
  caption: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
  },
  empty: {
    alignItems: 'flex-start',
    gap: spacingTokens['space.2'],
    paddingTop: spacingTokens['space.4'],
  },
  emptyTitle: {
    fontSize: typeTokens['a-heading'].size,
    lineHeight: typeTokens['a-heading'].lineHeight,
    fontWeight: '600',
  },
  emptyLine: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
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
  letter: {
    width: 20,
    textAlign: 'center',
    fontSize: typeTokens['a-label'].size,
    lineHeight: typeTokens['a-label'].lineHeight,
    fontWeight: '700',
  },
  names: {
    flex: 1,
  },
  name: {
    fontSize: typeTokens['a-body'].size,
    lineHeight: typeTokens['a-body'].lineHeight,
  },
  directory: {
    fontFamily: 'monospace',
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
  },
});
