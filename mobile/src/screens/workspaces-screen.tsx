import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { useEffect, useState, type ReactNode } from 'react';
import { RefreshControl, SectionList, StyleSheet, Text, View } from 'react-native';

import { FfiException, type Host, type WorkspaceSummary } from '../binding/types';
import { ActionButton } from '../components/action-button';
import { ConnectionBanner } from '../components/connection-banner';
import { LoadingRows } from '../components/loading-rows';
import { Overline } from '../components/overline';
import { StatePage } from '../components/state-page';
import { DETAILS, PAIR_AGAIN, RETRY, RETRY_NOW } from '../copy/connection';
import { ROUTINES_ACTION } from '../copy/routines';
import { LOADING_WORKSPACES, NO_WORKSPACES_LINE, NO_WORKSPACES_TITLE, ON_HOST, workspacesLoadFailed } from '../copy/workspaces';
import type { RootStackParamList } from '../navigation/routes';
import { connectionProblemOf } from '../state/connection-problem';
import { useHostRecord, useHostStore } from '../state/host-store-context';
import { formatAge } from '../state/relative-age';
import { useNow } from '../state/use-now';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';
import { connectionBannerModel, type BannerActionKind } from './connection-banner-model';
import { WorkspaceRowView } from './workspace-row';
import { workspaceSections, type WorkspaceRow } from './workspace-sections';

type Props = NativeStackScreenProps<RootStackParamList, 'Workspaces'>;

type ListState =
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly workspaces: readonly WorkspaceSummary[] }
  | { readonly status: 'error'; readonly message: string };

const BANNER_LABEL: Record<BannerActionKind, string> = {
  retry: RETRY,
  'retry-now': RETRY_NOW,
  details: DETAILS,
  'pair-again': PAIR_AGAIN,
};

/** M-PJ-01 (one host's workspaces, grouped by project), with each row's agent state and the connection banner. */
export function WorkspacesScreen({ navigation, route }: Props) {
  const { host } = route.params;
  const store = useHostStore();
  const palette = usePalette();
  const record = useHostRecord(host);
  const now = useNow(5_000);
  const [list, setList] = useState<ListState>({ status: 'loading' });
  const connection = record?.connection;
  const connectedHost = connection?.status === 'connected' ? connection.host : undefined;
  const agents = record?.agents;
  const linkLost = agents?.link.tag === 'Unverifiable';
  const age = agents?.lastContactMs === undefined ? undefined : formatAge(now - agents.lastContactMs);

  useEffect(() => {
    store.ensure(host);
  }, [host, store]);

  useEffect(() => {
    if (connection?.status === 'failed' && connectionProblemOf(connection.error).kind === 'protocol') {
      navigation.replace('ProtocolGate', { host });
    }
  }, [connection, host, navigation]);

  useEffect(() => {
    if (connectedHost === undefined) return undefined;
    let current = true;
    setList({ status: 'loading' });
    readWorkspaces(connectedHost).then((next) => {
      if (current) setList(next);
    });
    return () => {
      current = false;
    };
  }, [connectedHost]);

  function reload() {
    if (connectedHost === undefined) return;
    setList({ status: 'loading' });
    readWorkspaces(connectedHost).then(setList);
  }

  function runBannerAction(kind: BannerActionKind) {
    if (kind === 'details') navigation.navigate('ConnectionLog', { host });
    else if (kind === 'pair-again') navigation.navigate('Connect', { mode: 'scan' });
    else store.retry(host);
  }

  const banner = connectionBannerModel(record, host, now);

  let body: ReactNode;
  if (connection === undefined || connection.status === 'connecting') {
    body = <LoadingRows label={LOADING_WORKSPACES} />;
  } else if (connection.status === 'failed') {
    // A protocol mismatch leaves for the gate in the effect above; anything else is shown with the banner.
    body =
      connectionProblemOf(connection.error).kind === 'protocol' ? (
        <LoadingRows label={LOADING_WORKSPACES} />
      ) : (
        <StatePage title={workspacesLoadFailed(host)} detail={connection.error.message} />
      );
  } else if (list.status === 'loading') {
    body = <LoadingRows label={LOADING_WORKSPACES} />;
  } else if (list.status === 'error') {
    body = (
      <StatePage
        title={workspacesLoadFailed(host)}
        detail={list.message}
        actions={[{ label: RETRY, onPress: reload, variant: 'primary' }]}
      />
    );
  } else if (list.workspaces.length === 0) {
    body = <StatePage title={NO_WORKSPACES_TITLE} line={NO_WORKSPACES_LINE} />;
  } else {
    body = (
      <SectionList
        style={styles.flex}
        sections={workspaceSections(list.workspaces, agents?.sessions ?? [], linkLost, age)}
        keyExtractor={(row: WorkspaceRow) => row.id}
        renderSectionHeader={({ section }) => <Overline>{section.title}</Overline>}
        renderItem={({ item }) => (
          <WorkspaceRowView
            row={item}
            onOpen={() => navigation.navigate('Workspace', { host, workspace: item.id, noGit: item.noGit })}
          />
        )}
        refreshControl={<RefreshControl refreshing={false} onRefresh={reload} />}
        contentContainerStyle={styles.list}
      />
    );
  }

  return (
    <View style={[styles.screen, { backgroundColor: palette['surface.canvas'] }]}>
      {banner !== undefined && (
        <View style={styles.banner}>
          <ConnectionBanner
            tone={banner.tone}
            message={banner.message}
            actions={banner.actions.map((kind) => ({ label: BANNER_LABEL[kind], onPress: () => runBannerAction(kind) }))}
          />
        </View>
      )}
      <View style={styles.entries}>
        <Text style={[styles.caption, { color: palette['text.secondary'] }]}>{ON_HOST(host)}</Text>
        <ActionButton label={ROUTINES_ACTION} variant="ghost" onPress={() => navigation.navigate('Routines', { host })} />
      </View>
      {body}
    </View>
  );
}

function readWorkspaces(target: Host): Promise<ListState> {
  return target.workspaces().then(
    (workspaces): ListState => ({ status: 'ready', workspaces }),
    (error: unknown): ListState => ({ status: 'error', message: messageOf(error) }),
  );
}

function messageOf(error: unknown): string {
  return error instanceof FfiException ? error.error.message : String(error);
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
  },
  flex: {
    flex: 1,
  },
  banner: {
    padding: spacingTokens['space.4'],
  },
  entries: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingLeft: spacingTokens['space.4'],
    paddingRight: spacingTokens['space.2'],
  },
  list: {
    paddingBottom: spacingTokens['space.4'],
  },
  caption: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
  },
});
