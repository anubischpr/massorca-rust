export type PairMode = 'scan' | 'paste';

/** The compact navigation stack (M-SH-01, compact form). Each screen names the parameters it needs. */
export type RootStackParamList = {
  Home: undefined;
  Connect: { readonly mode?: PairMode } | undefined;
  ConfirmPairing: { readonly offer: string };
  ProtocolGate: { readonly host: string };
  Workspaces: { readonly host: string };
  /** `workspace` is the workspace the terminal belongs to, for its agent state. */
  Terminal: { readonly host: string; readonly terminal: string; readonly workspace?: string };
  Changes: undefined;
  ConnectionLog: { readonly host: string };
};
