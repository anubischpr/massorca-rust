/** The protocol versions this app speaks. Mirrors MIN_PROTOCOL_VERSION and PROTOCOL_VERSION in crates/protocol/src/lib.rs. */
export interface ProtocolRange {
  readonly min: number;
  readonly max: number;
}

export const APP_PROTOCOL: ProtocolRange = { min: 1, max: 1 };

/** The side that must update: the host when it speaks only versions below ours, the app when it needs newer ones. */
export type UpdateSide = 'host' | 'app' | 'unknown';

export function updateSideFor(
  host: { readonly min: number | undefined; readonly max: number | undefined },
  app: ProtocolRange = APP_PROTOCOL,
): UpdateSide {
  if (host.min === undefined || host.max === undefined) return 'unknown';
  if (host.max < app.min) return 'host';
  if (host.min > app.max) return 'app';
  return 'unknown';
}
