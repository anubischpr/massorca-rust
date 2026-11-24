// A stand-in for react-native-webview in Jest: it records what the app injects into the page, and a test posts what the
// page would post. Tests install it with jest.mock('react-native-webview', () => require('../terminal/test-webview')).

import { forwardRef, useImperativeHandle, useMemo } from 'react';
import { View } from 'react-native';

export interface PageMessageEvent {
  readonly nativeEvent: { readonly data: string };
}

export interface PageErrorEvent {
  readonly nativeEvent: { readonly description: string };
}

/** One mounted page: the scripts the app injected, and the handlers the app gave it. */
export class PageDouble {
  readonly injected: string[] = [];
  onMessage: ((event: PageMessageEvent) => void) | undefined;
  onError: ((event: PageErrorEvent) => void) | undefined;

  /** Posts a message as the page does, JSON-encoded. */
  post(message: object): void {
    this.onMessage?.({ nativeEvent: { data: JSON.stringify(message) } });
  }

  /** The engine failed to load the page. */
  fail(description: string): void {
    this.onError?.({ nativeEvent: { description } });
  }
}

/** Every page mounted so far, in order. */
export const webViews: PageDouble[] = [];

export interface WebViewDoubleProps {
  readonly onMessage?: (event: PageMessageEvent) => void;
  readonly onError?: (event: PageErrorEvent) => void;
  readonly style?: object;
}

const WebViewDouble = forwardRef<{ injectJavaScript(script: string): void }, WebViewDoubleProps>(
  ({ onMessage, onError, style }, ref) => {
    const page = useMemo(() => {
      const created = new PageDouble();
      webViews.push(created);
      return created;
    }, []);
    page.onMessage = onMessage;
    page.onError = onError;
    useImperativeHandle(ref, () => ({ injectJavaScript: (script: string) => page.injected.push(script) }), [page]);
    return <View style={style} testID="terminal-page" />;
  },
);

export default WebViewDouble;
