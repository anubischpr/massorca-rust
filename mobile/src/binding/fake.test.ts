import { FakeMassOrca } from './fake';
import { errorCode, errorFromCode, isTransient } from './ffi-error';
import { FfiException, type FfiError, type HostSummary } from './types';

const offer = 'fake-offer:example';

const home: HostSummary = {
  name: 'home-pc',
  server: 'vps.example.ts.net',
  addresses: ['100.64.0.10:7443'],
  pin: 'sha256:fake',
  deviceName: 'fake-phone',
  scopes: [],
  pairedAtMs: 1n,
};

async function caught(promise: Promise<unknown>): Promise<FfiError> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof FfiException) return error.error;
    throw error;
  }
  throw new Error('expected the call to fail');
}

describe('FakeMassOrca', () => {
  it('lists the hosts it was given and records each call', () => {
    const fake = new FakeMassOrca({ hosts: [home] });

    expect(fake.hosts()).toEqual([home]);
    expect(fake.calls).toEqual(['hosts']);
  });

  it('pairs a valid offer, and refuses one that is not an offer', async () => {
    const fake = new FakeMassOrca();

    const paired = await fake.pair(offer, 'home-pc');
    expect(paired.name).toBe('home-pc');
    expect(fake.hosts().map((h) => h.name)).toEqual(['home-pc']);

    expect(await caught(fake.pair('not a code', 'other'))).toEqual({
      tag: 'InvalidOffer',
      message: 'Not a valid pairing code.',
    });
  });

  it('refuses a second host with the same name as HostExists', async () => {
    const fake = new FakeMassOrca({ hosts: [home] });

    expect((await caught(fake.pair(offer, 'home-pc'))).tag).toBe('HostExists');
  });

  it('returns a scripted pair result, or its scripted error, before the default', async () => {
    const fake = new FakeMassOrca();
    fake.queuePair({ tag: 'Unreachable', message: 'This host can not be reached.' });
    fake.queuePair(home);

    expect((await caught(fake.pair(offer, 'home-pc'))).tag).toBe('Unreachable');
    expect(await fake.pair(offer, 'home-pc')).toBe(home);
  });

  it('fails the next call of a method with the case its code names', async () => {
    const fake = new FakeMassOrca({ hosts: [home] });
    fake.failNext('hosts', 'storage', 'The pairing key could not be read.');

    const error = await caught(Promise.resolve().then(() => fake.hosts()));
    expect(error).toEqual({ tag: 'Storage', message: 'The pairing key could not be read.' });
    expect(fake.hosts()).toEqual([home]);
  });

  it('reports HostNotFound when connecting to or forgetting an unknown host', async () => {
    const fake = new FakeMassOrca();

    expect((await caught(fake.connect('nope'))).tag).toBe('HostNotFound');
    expect(() => fake.forget('nope')).toThrow(FfiException);
  });

  it('removes a forgotten host', () => {
    const fake = new FakeMassOrca({ hosts: [home] });
    fake.forget('home-pc');

    expect(fake.hosts()).toEqual([]);
  });

  it('delivers the scripted link changes of a watch, then ends it', async () => {
    const fake = new FakeMassOrca({ hosts: [home] });
    const unverifiable = {
      tag: 'Link' as const,
      link: { tag: 'Unverifiable' as const, attempt: 1, retryInMs: 2000n, reason: 'connection closed' },
    };
    fake.scriptGitWatch([unverifiable, { tag: 'Link', link: { tag: 'Live' } }]);

    const host = await fake.connect('home-pc');
    const watch = host.watchGit('workspace-1');
    expect(await watch.next()).toEqual(unverifiable);
    expect(await watch.next()).toEqual({ tag: 'Link', link: { tag: 'Live' } });
    expect(await watch.next()).toBeNull();
  });
});

describe('ffi error codes', () => {
  const cases: FfiError[] = [
    { tag: 'Unreachable', message: 'm' },
    { tag: 'IdentityMismatch', message: 'm' },
    { tag: 'NotPaired', message: 'm' },
    { tag: 'ProtocolMismatch', message: 'm', supportedMin: undefined, supportedMax: undefined },
    { tag: 'InvalidOffer', message: 'm' },
    { tag: 'HostExists', message: 'm' },
    { tag: 'HostNotFound', message: 'm' },
    { tag: 'InvalidHostName', message: 'm' },
    { tag: 'Storage', message: 'm' },
    { tag: 'Internal', message: 'm' },
    { tag: 'Remote', code: 'path.not_found', message: 'm' },
  ];

  it('round-trips every case through its stable code', () => {
    for (const error of cases) {
      expect(errorFromCode(errorCode(error), 'm')).toEqual(error);
    }
  });

  it('marks only an unreachable host and a shutting-down service as transient', () => {
    expect(cases.filter(isTransient).map((e) => e.tag)).toEqual(['Unreachable']);
    expect(isTransient({ tag: 'Remote', code: 'service.shutting_down', message: 'm' })).toBe(true);
  });
});
