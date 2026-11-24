// Screen 6, the terminal (M-TM-01 to M-TM-03): one terminal of a host, shown by the xterm.js page in a WebView. The app
// attaches to the terminal at the size the page fits, forwards the output to the page, and sends what the user types.

import { useEffect, useRef, useState, type ReactNode } from 'react';
import { StyleSheet, View } from 'react-native';

import { FfiException, type TerminalEvent, type TerminalView } from '../binding/types';
import { encodeUtf8 } from '../binding/utf8';
import { AgentStatusPill } from '../components/agent-status-pill';
import { ConnectionBanner, type BannerTone } from '../components/connection-banner';
import { StatePage } from '../components/state-page';
import { DETAILS, RETRY } from '../copy/connection';
import { ATTACH_FAILED_TITLE, ENGINE_FAILED_LINE, ENGINE_FAILED_TITLE, exitedLine, LOADING_TERMINAL } from '../copy/terminal';
import { aggregateDisplay } from '../state/agent-display';
import { useHostRecord, useHostStore } from '../state/host-store-context';
import { formatAge } from '../state/relative-age';
import { useNow } from '../state/use-now';
import { applyControl } from '../terminal/control-key';
import { PageView, type PageHandle } from '../terminal/page-view';
import {
  keyScript,
  parsePageMessage,
  readOnlyScript,
  resizeScript,
  restoreScript,
  TERMINAL_PAGE_URI,
  terminalTheme,
  themeScript,
  writeScript,
} from '../terminal/terminal-bridge';
import { applyTerminalEvent, INITIAL_TERMINAL_SESSION, type TerminalSession } from '../terminal/terminal-session';
import { usePalette } from '../theme/palette';
import { spacingTokens } from '../theme/tokens.generated';
import { glyphViewOf } from './agent-display-view';
import { linkBannerModel } from './link-banner-model';
import { TerminalInputDock, TerminalKeyRow, type KeyName } from './terminal-input';

export interface TerminalScreenProps {
  readonly host: string;
  readonly terminal: string;
  /** The workspace the terminal belongs to; the status pill shows its agent state. */
  readonly workspace?: string;
  readonly onOpenDetails: () => void;
}

/** The page is ready once it has fitted its size; until then nothing is attached. */
type PageState =
  | { readonly tag: 'loading' }
  | { readonly tag: 'ready'; readonly cols: number; readonly rows: number }
  | { readonly tag: 'failed'; readonly detail: string };

interface Size {
  readonly cols: number;
  readonly rows: number;
}

/** The notice above the terminal; `details` opens the connection log. */
interface Notice {
  readonly tone: BannerTone;
  readonly message: string;
  readonly details: boolean;
}

export function TerminalScreen({ host, terminal, workspace, onOpenDetails }: TerminalScreenProps) {
  const store = useHostStore();
  const palette = usePalette();
  const record = useHostRecord(host);
  const now = useNow(1_000);
  const [page, setPage] = useState<PageState>({ tag: 'loading' });
  const [pageKey, setPageKey] = useState(0);
  const [attempt, setAttempt] = useState(0);
  const [session, setSession] = useState<TerminalSession>(INITIAL_TERMINAL_SESSION);
  const [attachError, setAttachError] = useState<string | undefined>(undefined);
  const [ctrlArmed, setCtrlArmed] = useState(false);
  const [typed, setTyped] = useState('');
  const webRef = useRef<PageHandle>(null);
  const viewRef = useRef<TerminalView | undefined>(undefined);
  const ctrlRef = useRef(false);
  const sizeRef = useRef<Size | undefined>(undefined);

  const connection = record?.connection;
  const connected = connection?.status === 'connected' ? connection.host : undefined;
  const pageReady = page.tag === 'ready';
  const readOnly = !pageReady || session.phase.tag === 'exited' || session.link.tag === 'Unverifiable';

  useEffect(() => {
    store.ensure(host);
  }, [host, store]);

  // The page's size is the size the terminal opens at, so the host's first snapshot already fits it.
  useEffect(() => {
    if (connected === undefined || page.tag !== 'ready') return undefined;
    let current = true;
    let opened: TerminalView | undefined;
    setSession(INITIAL_TERMINAL_SESSION);
    setAttachError(undefined);
    sizeRef.current = { cols: page.cols, rows: page.rows };

    const deliver = (step: TerminalEvent): void => {
      const at = Date.now();
      setSession((previous) => applyTerminalEvent(previous, step, at));
      if (step.tag === 'Output') webRef.current?.injectJavaScript(writeScript(step.bytes));
      if (step.tag === 'Snapshot') {
        sizeRef.current = { cols: step.cols, rows: step.rows };
        webRef.current?.injectJavaScript(restoreScript(step.cols, step.rows, step.bytes));
      }
      if (step.tag === 'Resized') {
        sizeRef.current = { cols: step.cols, rows: step.rows };
        webRef.current?.injectJavaScript(resizeScript(step.cols, step.rows));
      }
    };

    connected.attachTerminal(terminal, page.cols, page.rows).then(
      async (view) => {
        if (!current) {
          view.close();
          return;
        }
        opened = view;
        viewRef.current = view;
        for (let event = await view.next(); event !== null && current; event = await view.next()) deliver(event);
      },
      (error: unknown) => {
        if (current) setAttachError(messageOf(error));
      },
    );
    return () => {
      current = false;
      opened?.close();
      viewRef.current = undefined;
    };
  }, [connected, page, terminal, attempt]);

  // The page takes no keystrokes while the host is out of contact or the process has ended (xterm also drops its cursor
  // replies then; the host answers the queries it holds when the view comes back).
  useEffect(() => {
    if (pageReady) webRef.current?.injectJavaScript(readOnlyScript(readOnly));
  }, [pageReady, readOnly]);

  useEffect(() => {
    if (pageReady) webRef.current?.injectJavaScript(themeScript(terminalTheme(palette)));
  }, [palette, pageReady]);

  function setArmed(armed: boolean): void {
    ctrlRef.current = armed;
    setCtrlArmed(armed);
  }

  function sendBytes(bytes: ArrayBuffer): void {
    // Input is refused while the host is out of contact; the notice above says so, so the refusal is not shown twice.
    viewRef.current?.input(bytes).catch(() => undefined);
  }

  function onPageMessage(raw: string): void {
    const message = parsePageMessage(raw);
    if (message === undefined) return;
    switch (message.kind) {
      case 'ready':
        setPage({ tag: 'ready', cols: message.cols, rows: message.rows });
        return;
      case 'input': {
        const armed = ctrlRef.current;
        if (armed) setArmed(false);
        sendBytes(encodeUtf8(armed ? applyControl(message.text) : message.text));
        return;
      }
      case 'resize': {
        const known = sizeRef.current;
        if (known?.cols === message.cols && known.rows === message.rows) return;
        sizeRef.current = { cols: message.cols, rows: message.rows };
        viewRef.current?.resize(message.cols, message.rows).catch(() => undefined);
        return;
      }
      case 'error':
        setPage({ tag: 'failed', detail: message.message });
        return;
    }
  }

  function onKey(key: KeyName): void {
    if (key === 'ctrl') {
      setArmed(!ctrlRef.current);
      return;
    }
    webRef.current?.injectJavaScript(keyScript(key));
  }

  function sendLine(): void {
    sendBytes(encodeUtf8(`${typed}\r`));
    setTyped('');
  }

  function retryEngine(): void {
    setPage({ tag: 'loading' });
    setPageKey((key) => key + 1);
    setAttempt((count) => count + 1);
  }

  function retryAttach(): void {
    setAttachError(undefined);
    setAttempt((count) => count + 1);
  }

  const failure = failureView();
  const notice = failure === undefined ? noticeOf() : undefined;
  const agents = record?.agents;
  const linkLost = agents?.link.tag === 'Unverifiable';
  const sessions = (agents?.sessions ?? []).filter((item) => item.workspaceId === workspace);
  const agentView =
    workspace === undefined
      ? undefined
      : glyphViewOf(
          aggregateDisplay(sessions, linkLost),
          agents?.lastContactMs === undefined ? undefined : formatAge(now - agents.lastContactMs),
        );
  const keysDisabled = readOnly || failure !== undefined;

  return (
    <View style={[styles.screen, { backgroundColor: palette['surface.canvas'] }]}>
      {notice !== undefined && (
        <View style={styles.notice}>
          <ConnectionBanner
            tone={notice.tone}
            message={notice.message}
            actions={notice.details ? [{ label: DETAILS, onPress: onOpenDetails }] : []}
          />
        </View>
      )}
      {agentView !== undefined && (
        <View style={styles.pill}>
          <AgentStatusPill glyph={agentView.glyph} word={agentView.word} />
        </View>
      )}
      <View style={styles.content}>
        {page.tag !== 'failed' && (
          <PageView
            key={pageKey}
            ref={webRef}
            originWhitelist={['file://*']}
            onError={(event) => setPage({ tag: 'failed', detail: event.nativeEvent.description })}
            onMessage={(event) => onPageMessage(event.nativeEvent.data)}
            onShouldStartLoadWithRequest={(request) => request.url === TERMINAL_PAGE_URI}
            source={{ uri: TERMINAL_PAGE_URI }}
            style={styles.page}
          />
        )}
        {failure !== undefined && <View style={styles.overlay}>{failure}</View>}
      </View>
      <TerminalKeyRow ctrlArmed={ctrlArmed} disabled={keysDisabled} onKey={onKey} />
      <TerminalInputDock disabled={keysDisabled} onChangeText={setTyped} onSend={sendLine} value={typed} />
    </View>
  );

  /** What replaces the terminal: the engine failed, the attach was refused, or the connection failed. */
  function failureView(): ReactNode {
    if (page.tag === 'failed') {
      return (
        <StatePage
          title={ENGINE_FAILED_TITLE}
          line={ENGINE_FAILED_LINE(host)}
          detail={page.detail}
          actions={[{ label: RETRY, onPress: retryEngine, variant: 'primary' }]}
        />
      );
    }
    if (attachError !== undefined) {
      return (
        <StatePage
          title={ATTACH_FAILED_TITLE}
          detail={attachError}
          actions={[{ label: RETRY, onPress: retryAttach, variant: 'primary' }]}
        />
      );
    }
    if (connection?.status === 'failed') {
      return (
        <StatePage
          title={ATTACH_FAILED_TITLE}
          detail={connection.error.message}
          actions={[{ label: RETRY, onPress: () => store.retry(host), variant: 'primary' }]}
        />
      );
    }
    return undefined;
  }

  /** The exit of the process, the pending attach, or what the link is doing. */
  function noticeOf(): Notice | undefined {
    if (session.phase.tag === 'exited') {
      return { tone: 'info', message: exitedLine(session.phase.code, session.phase.signal), details: false };
    }
    if (session.phase.tag === 'attaching') {
      return { tone: 'info', message: LOADING_TERMINAL(host), details: false };
    }
    const link = linkBannerModel(host, session, now);
    if (link === undefined) return undefined;
    return { tone: link.tone, message: link.message, details: link.actions.length > 0 };
  }
}

function messageOf(error: unknown): string {
  return error instanceof FfiException ? error.error.message : String(error);
}

const styles = StyleSheet.create({
  screen: {
    flex: 1,
  },
  notice: {
    padding: spacingTokens['space.3'],
  },
  pill: {
    paddingHorizontal: spacingTokens['space.3'],
    paddingBottom: spacingTokens['space.2'],
  },
  content: {
    flex: 1,
  },
  page: {
    flex: 1,
  },
  overlay: {
    position: 'absolute',
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
  },
});
