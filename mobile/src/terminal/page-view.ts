import type { FunctionComponent, RefAttributes } from 'react';
import type { StyleProp, ViewStyle } from 'react-native';
import WebView from 'react-native-webview';

/** The calls the screen makes on the page's WebView. */
export interface PageHandle {
  injectJavaScript(script: string): void;
}

/** The props the terminal page uses: the library's own prop types, narrowed to these. */
export interface PageViewProps {
  readonly originWhitelist: readonly string[];
  readonly source: { readonly uri: string };
  readonly style?: StyleProp<ViewStyle>;
  readonly onError: (event: { readonly nativeEvent: { readonly description: string } }) => void;
  readonly onMessage: (event: { readonly nativeEvent: { readonly data: string } }) => void;
  readonly onShouldStartLoadWithRequest: (request: { readonly url: string }) => boolean;
}

// SAFETY: the library's own prop types do not fit this TypeScript setup, so the props the page uses are declared here.
// The component is the library's WebView, which takes these props and passes its ref on to the handle.
export const PageView = WebView as unknown as FunctionComponent<PageViewProps & RefAttributes<PageHandle>>;
