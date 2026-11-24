export const PROTOCOL_CHECK_AGAIN = 'Check again';
export const PROTOCOL_OPEN_STORE = 'Open Google Play';
export const PROTOCOL_BACK_TO_HOSTS = 'Back to hosts';

/** M-SH-02 copy. The desktop side uses the host name the phone gave it. */
export const PROTOCOL_COPY = {
  host: {
    title: (host: string) => `${host} needs an update`,
    body: (host: string, hostMax: number, appMin: number) =>
      `MassOrca on ${host} speaks protocol ${hostMax}. This app needs protocol ${appMin} or newer. Update MassOrca on ${host}, then check again.`,
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
