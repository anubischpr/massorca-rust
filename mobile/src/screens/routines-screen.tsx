import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { useEffect, useState, type ReactNode } from 'react';
import { SectionList, StyleSheet, Text, View } from 'react-native';

import { FfiException, type RoutineSummary } from '../binding/types';
import { ConnectionBanner } from '../components/connection-banner';
import { LoadingRows } from '../components/loading-rows';
import { Overline } from '../components/overline';
import { StatePage } from '../components/state-page';
import { DETAILS, PAIR_AGAIN, RETRY, RETRY_NOW } from '../copy/connection';
import {
  LOADING_ROUTINES,
  NOT_CHANGED,
  NO_ROUTINES_LINE,
  NO_ROUTINES_TITLE,
  ROUTINES_NOT_SUPPORTED_LINE,
  ROUTINES_NOT_SUPPORTED_TITLE,
  routinesLoadFailed,
} from '../copy/routines';
import { ON_HOST } from '../copy/workspaces';
import type { RootStackParamList } from '../navigation/routes';
import { connectionProblemOf } from '../state/connection-problem';
import { useHostRecord, useHostStore } from '../state/host-store-context';
import { useNow } from '../state/use-now';
import { usePalette } from '../theme/palette';
import { spacingTokens, typeTokens } from '../theme/tokens.generated';
import { connectionBannerModel, type BannerActionKind } from './connection-banner-model';
import { RoutineRowView } from './routine-row';
import { loadRoutines, type RoutinesLoad } from './routines-load';
import { replaceRoutine, routineRowText, type RoutineEntry } from './routines-model';

type Props = NativeStackScreenProps<RootStackParamList, 'Routines'>;

type ScreenState = { readonly status: 'loading' } | RoutinesLoad;

interface RoutineListSection {
  readonly key: string;
  readonly title: string;
  readonly data: readonly RoutineEntry[];
}

const BANNER_LABEL: Record<BannerActionKind, string> = {
  retry: RETRY,
  'retry-now': RETRY_NOW,
  details: DETAILS,
  'pair-again': PAIR_AGAIN,
};

/**
 * The routines of the host's teams (AUTO-28; section 11.1 of docs/design/screens/android.md). Each row pauses and resumes
 * its routine; the connection banner and the last known state follow the host as they do on the Projects screen.
 */
export function RoutinesScreen({ navigation, route }: Props) {
  const { host } = route.params;
  const store = useHostStore();
  const palette = usePalette();
  const record = useHostRecord(host);
  const now = useNow(5_000);
  const [state, setState] = useState<ScreenState>({ status: 'loading' });
  const [busy, setBusy] = useState<string | undefined>(undefined);
  const [notice, setNotice] = useState<string | undefined>(undefined);
  const connection = record?.connection;
  const connectedHost = connection?.status === 'connected' ? connection.host : undefined;

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
    setState({ status: 'loading' });
    loadRoutines(connectedHost).then((next) => {
      if (current) setState(next);
    });
    return () => {
      current = false;
    };
  }, [connectedHost]);

  function reload() {
    if (connectedHost === undefined) return;
    setState({ status: 'loading' });
    loadRoutines(connectedHost).then(setState);
  }

  function runBannerAction(kind: BannerActionKind) {
    if (kind === 'details') navigation.navigate('ConnectionLog', { host });
    else if (kind === 'pair-again') navigation.navigate('Connect', { mode: 'scan' });
    else store.retry(host);
  }

  /** Pauses or resumes one routine. The row shows the host's answer; a refusal keeps the row and names the reason. */
  function setPaused(teamId: string, routine: RoutineSummary, paused: boolean) {
    if (connectedHost === undefined) return;
    setBusy(routine.id);
    setNotice(undefined);
    const change = paused
      ? connectedHost.pauseRoutine(teamId, routine.id)
      : connectedHost.resumeRoutine(teamId, routine.id);
    change.then(
      (updated) => {
        setState((current) => (current.status === 'ready' ? { status: 'ready', sections: replaceRoutine(current.sections, updated) } : current));
        setBusy(undefined);
      },
      (error: unknown) => {
        setNotice(NOT_CHANGED(messageOf(error)));
        setBusy(undefined);
      },
    );
  }

  const banner = connectionBannerModel(record, host, now);

  let body: ReactNode;
  if (connection === undefined || connection.status === 'connecting') {
    body = <LoadingRows label={LOADING_ROUTINES} />;
  } else if (connection.status === 'failed') {
    // A protocol mismatch leaves for the gate in the effect above; anything else is shown with the banner.
    body =
      connectionProblemOf(connection.error).kind === 'protocol' ? (
        <LoadingRows label={LOADING_ROUTINES} />
      ) : (
        <StatePage title={routinesLoadFailed(host)} detail={connection.error.message} />
      );
  } else if (state.status === 'loading') {
    body = <LoadingRows label={LOADING_ROUTINES} />;
  } else if (state.status === 'unsupported') {
    body = <StatePage title={ROUTINES_NOT_SUPPORTED_TITLE} line={ROUTINES_NOT_SUPPORTED_LINE} />;
  } else if (state.status === 'error') {
    body = (
      <StatePage
        title={routinesLoadFailed(host)}
        detail={state.message}
        actions={[{ label: RETRY, onPress: reload, variant: 'primary' }]}
      />
    );
  } else if (state.sections.length === 0) {
    body = <StatePage title={NO_ROUTINES_TITLE} line={NO_ROUTINES_LINE} />;
  } else {
    const sections: readonly RoutineListSection[] = state.sections.map((section) => ({
      key: section.teamId,
      title: section.teamName,
      data: section.routines,
    }));
    body = (
      <SectionList
        style={styles.flex}
        sections={sections}
        keyExtractor={(entry: RoutineEntry) => entry.routine.id}
        renderSectionHeader={({ section }) => <Overline>{section.title}</Overline>}
        renderItem={({ item, section }) => (
          <RoutineRowView
            text={routineRowText(item, now)}
            busy={busy === item.routine.id}
            onPausedChange={(paused) => setPaused(section.key, item.routine, paused)}
          />
        )}
        ListHeaderComponent={
          <View style={styles.header}>
            <Text style={[styles.caption, { color: palette['text.secondary'] }]}>{ON_HOST(host)}</Text>
            {notice !== undefined && <Text style={[styles.caption, { color: palette['status.danger.fg'] }]}>{notice}</Text>}
          </View>
        }
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
      {body}
    </View>
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
  header: {
    paddingHorizontal: spacingTokens['space.4'],
    paddingBottom: spacingTokens['space.2'],
    gap: spacingTokens['space.1'],
  },
  list: {
    paddingBottom: spacingTokens['space.4'],
  },
  caption: {
    fontSize: typeTokens['a-caption'].size,
    lineHeight: typeTokens['a-caption'].lineHeight,
  },
});
