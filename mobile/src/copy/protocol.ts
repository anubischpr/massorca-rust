import { APP_PROTOCOL, type UpdateSide } from '../state/app-protocol';

export const PROTOCOL_CHECK_AGAIN = 'Check again';
export const PROTOCOL_OPEN_STORE = 'Open Google Play';
export const PROTOCOL_BACK_TO_HOSTS = 'Back to hosts';

/** M-SH-02 copy. The desktop is named by the host name the phone gave it. */
export const PROTOCOL_COPY = {
  host: {
    title: (host: string) => `${host} needs an update`,
    body: (host: string, hostMax: number) =>
      `MassOrca on ${host} speaks protocol ${hostMax}. This app needs protocol ${APP_PROTOCOL.min} or newer. Update MassOrca on ${host}, then check again.`,
  },
  app: {
    title: 'This app needs an update',
    body: (host: string, hostMin: number) =>
      `${host} speaks protocol ${hostMin}. Update MassOrca from Google Play, then open it again.`,
  },
  unknown: {
    title: 'The two versions do not match',
    body: (host: string) =>
      `This app and ${host} speak different protocol versions. Update the older MassOrca, then check again.`,
  },
} as const;

export const PROTOCOL_CHECK_FAILED = (host: string): string => `Could not check ${host}`;

/** "protocol 1" or "protocols 1 to 3"; "unknown" when the host did not say. */
export function protocolRangeText(min: number | undefined, max: number | undefined): string {
  if (min === undefined || max === undefined) return 'unknown';
  return min === max ? `protocol ${min}` : `protocols ${min} to ${max}`;
}

/** The gate's title and body for the side that must update; without the numbers it falls back to the neutral text. */
export function protocolGateText(
  side: UpdateSide,
  host: string,
  hostMin: number | undefined,
  hostMax: number | undefined,
): { readonly title: string; readonly body: string } {
  if (side === 'host' && hostMax !== undefined) {
    return { title: PROTOCOL_COPY.host.title(host), body: PROTOCOL_COPY.host.body(host, hostMax) };
  }
  if (side === 'app' && hostMin !== undefined) {
    return { title: PROTOCOL_COPY.app.title, body: PROTOCOL_COPY.app.body(host, hostMin) };
  }
  return { title: PROTOCOL_COPY.unknown.title, body: PROTOCOL_COPY.unknown.body(host) };
}
