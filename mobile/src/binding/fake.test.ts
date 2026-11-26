import { FakeMassOrca } from './fake';
import { errorCode, errorFromCode, isTransient } from './ffi-error';
import { FfiException, type FfiError, type HostSummary, type TerminalSummary } from './types';

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

function terminal(id: string, workspaceId: string): TerminalSummary {
  return {
    id,
    workspaceId,
    state: 'live',
    title: undefined,
    cols: 120,
    rows: 40,
    shell: 'zsh',
    cwd: '/repos/ws-1',
    createdAtMs: 1n,
    holdsSecrets: false,
  };
}

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

  it('attaches a scripted terminal: it delivers the script, then stays open until it is closed', async () => {
    const fake = new FakeMassOrca({ hosts: [home] });
    fake.scriptTerminal([{ tag: 'Resized', cols: 100, rows: 30 }]);
    const view = await (await fake.connect('home-pc')).attachTerminal('term-1', 80, 24);

    expect(fake.attaches).toEqual([{ terminal: 'term-1', cols: 80, rows: 24 }]);
    expect(await view.next()).toEqual({ tag: 'Resized', cols: 100, rows: 30 });

    const pending = view.next();
    fake.terminals[0]?.push({ tag: 'Link', link: { tag: 'Live' } });
    expect(await pending).toEqual({ tag: 'Link', link: { tag: 'Live' } });

    view.close();
    expect(await view.next()).toBeNull();
  });

  it('ends a terminal view after its exit, as the host does', async () => {
    const fake = new FakeMassOrca({ hosts: [home] });
    fake.scriptTerminal([{ tag: 'Exited', reason: 'exited', code: 1, signal: undefined }]);
    const view = await (await fake.connect('home-pc')).attachTerminal('term-1', 80, 24);

    expect(await view.next()).toEqual({ tag: 'Exited', reason: 'exited', code: 1, signal: undefined });
    fake.terminals[0]?.push({ tag: 'Resized', cols: 1, rows: 1 });
    expect(await view.next()).toBeNull();
  });

  it('records the input and the resizes sent to a terminal view, and refuses them once it is closed', async () => {
    const fake = new FakeMassOrca({ hosts: [home] });
    const view = await (await fake.connect('home-pc')).attachTerminal('term-1', 80, 24);

    await view.input(new TextEncoder().encode('ls').buffer);
    await view.resize(120, 40);
    expect(fake.terminals[0]?.resizes).toEqual([[120, 40]]);
    expect(fake.terminals[0]?.inputs.map((bytes) => new TextDecoder().decode(bytes))).toEqual(['ls']);

    view.close();
    await expect(view.resize(80, 24)).rejects.toBeInstanceOf(FfiException);
  });

  it('fails a scripted attach with its error, and returns the git status it is set to', async () => {
    const fake = new FakeMassOrca({ hosts: [home] });
    const host = await fake.connect('home-pc');
    fake.failNext('attachTerminal', 'unreachable', 'home-pc is off');
    await expect(host.attachTerminal('term-1', 80, 24)).rejects.toMatchObject({ error: { tag: 'Unreachable' } });
    expect(fake.attaches).toEqual([]);

    const status = { branch: 'main', ahead: 1n, behind: undefined, operation: undefined, files: [], truncated: false };
    fake.setGitStatus(status);
    expect(await host.gitStatus('workspace-1')).toEqual(status);
  });

  it('lists the terminals of one workspace, or of every workspace, and records each call', async () => {
    const fake = new FakeMassOrca({ hosts: [home], terminals: [terminal('t-1', 'ws-1'), terminal('t-2', 'ws-2')] });
    const host = await fake.connect('home-pc');

    expect((await host.terminals('ws-1')).map((t) => t.id)).toEqual(['t-1']);
    expect((await host.terminals()).map((t) => t.id)).toEqual(['t-1', 't-2']);
    expect(fake.calls.filter((call) => call === 'terminals')).toHaveLength(2);
  });

  it('fails the terminal list once with the scripted error, then lists again', async () => {
    const fake = new FakeMassOrca({ hosts: [home], terminals: [terminal('t-1', 'ws-1')] });
    fake.failNext('terminals', 'unreachable', 'home-pc is off');
    const host = await fake.connect('home-pc');

    expect(await caught(host.terminals('ws-1'))).toEqual({ tag: 'Unreachable', message: 'home-pc is off' });
    expect((await host.terminals('ws-1')).map((t) => t.id)).toEqual(['t-1']);
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
