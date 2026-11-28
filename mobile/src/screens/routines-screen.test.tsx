import { fireEvent, render, screen } from '@testing-library/react-native';

import App from '../../App';
import { FakeMassOrca } from '../binding/fake';
import { bot, hostSummary, routine, routineRun, team } from '../state/test-fixtures';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

const TEAMS_CAPABILITIES = { protocol: 16, serverName: 'home', serverVersion: '0.9.4', capabilities: ['teams/1'] };

/** Opens the host's Projects screen from Home, then its Routines. */
async function openRoutines(fake: FakeMassOrca, badge: string = 'Connected') {
  await render(<App binding={fake} />);
  await fireEvent.press(await screen.findByRole('button', { name: `home-pc, ${badge}` }));
  await fireEvent.press(await screen.findByRole('button', { name: 'Routines' }));
}

function switchOf(name: string) {
  return screen.getByRole('switch', { name });
}

describe('routines of a host', () => {
  it("lists each team's routines with its schedule, its bot and the newest run", async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      protocol: TEAMS_CAPABILITIES,
      teams: [team({ id: 'team-1', name: 'alpha' })],
      bots: [bot({ id: 'bot-1', name: 'mara' })],
      routines: [routine({ id: 'morning', name: 'morning' })],
      routineRuns: [routineRun({ id: 'run-1', routineId: 'morning' })],
    });
    await openRoutines(fake);

    expect(await screen.findByText('alpha')).toBeTruthy();
    expect(screen.getByText('morning')).toBeTruthy();
    expect(screen.getByText(/^Every Monday at 09:00, mara, next in \d+ days$/)).toBeTruthy();
    expect(screen.getByText('Last run 2 h ago')).toBeTruthy();
    expect(switchOf('morning, running')).toBeTruthy();
  });

  it('says there are no routines yet when no team has one', async () => {
    await openRoutines(new FakeMassOrca({ hosts: [hostSummary()], protocol: TEAMS_CAPABILITIES, teams: [team({ id: 'team-1' })] }));

    expect(await screen.findByText('No routines yet')).toBeTruthy();
    expect(screen.getByText('A routine starts a bot on a task at a time you set. Make one on This PC.')).toBeTruthy();
  });

  it('shows a loading state until the routines arrive', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], protocol: TEAMS_CAPABILITIES, teams: [team({ id: 'team-1' })] });
    fake.stallNextTeams();
    await openRoutines(fake);

    expect(await screen.findByLabelText('Loading routines')).toBeTruthy();
  });

  it('shows a paused routine with its switch off and no next run', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      protocol: TEAMS_CAPABILITIES,
      teams: [team({ id: 'team-1', name: 'alpha' })],
      bots: [bot({ id: 'bot-1', name: 'mara' })],
      routines: [routine({ id: 'morning', name: 'morning', paused: true, nextRunAtMs: undefined })],
    });
    await openRoutines(fake);

    expect(await screen.findByText('Every Monday at 09:00, mara, paused')).toBeTruthy();
    expect(switchOf('morning, paused')).toBeTruthy();
  });

  it('resumes a paused routine when its switch is turned on, and shows its next run', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      protocol: TEAMS_CAPABILITIES,
      teams: [team({ id: 'team-1', name: 'alpha' })],
      bots: [bot({ id: 'bot-1', name: 'mara' })],
      routines: [routine({ id: 'morning', name: 'morning', paused: true, nextRunAtMs: undefined })],
    });
    await openRoutines(fake);

    fireEvent(await screen.findByRole('switch', { name: 'morning, paused' }), 'valueChange', true);

    expect(await screen.findByText(/^Every Monday at 09:00, mara, next in \d+ days$/)).toBeTruthy();
    expect(fake.calls).toContain('resumeRoutine');
  });

  it('pauses a running routine when its switch is turned off', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      protocol: TEAMS_CAPABILITIES,
      teams: [team({ id: 'team-1', name: 'alpha' })],
      bots: [bot({ id: 'bot-1', name: 'mara' })],
      routines: [routine({ id: 'morning', name: 'morning' })],
    });
    await openRoutines(fake);

    fireEvent(await screen.findByRole('switch', { name: 'morning, running' }), 'valueChange', false);

    expect(await screen.findByText('Every Monday at 09:00, mara, paused')).toBeTruthy();
    expect(fake.calls).toContain('pauseRoutine');
  });

  it('keeps the row as it was and names the reason when the host refuses the change', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      protocol: TEAMS_CAPABILITIES,
      teams: [team({ id: 'team-1', name: 'alpha' })],
      bots: [bot({ id: 'bot-1', name: 'mara' })],
      routines: [routine({ id: 'morning', name: 'morning' })],
    });
    fake.failNext('pauseRoutine', 'internal', 'The routine is busy.');
    await openRoutines(fake);

    fireEvent(await screen.findByRole('switch', { name: 'morning, running' }), 'valueChange', false);

    expect(await screen.findByText('Not changed: The routine is busy.')).toBeTruthy();
    expect(screen.getByText(/^Every Monday at 09:00, mara, next in \d+ days$/)).toBeTruthy();
  });

  it('shows an error when the routines cannot be read, and retries them', async () => {
    const fake = new FakeMassOrca({
      hosts: [hostSummary()],
      protocol: TEAMS_CAPABILITIES,
      teams: [team({ id: 'team-1', name: 'alpha' })],
      routines: [routine({ id: 'morning', name: 'morning' })],
    });
    fake.failNext('teamRoutines', 'internal', 'Unexpected data from the host.');
    await openRoutines(fake);

    expect(await screen.findByText('Could not load routines from home-pc')).toBeTruthy();
    expect(screen.getByText('Unexpected data from the host.')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));

    expect(await screen.findByText('morning')).toBeTruthy();
  });

  it('shows a host that cannot be reached with the banner, and retries the connection', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], protocol: TEAMS_CAPABILITIES, teams: [team({ id: 'team-1' })] });
    // One refusal for each attempt the screens make on the way here: Home, the Projects screen and this one.
    for (let attempt = 0; attempt < 4; attempt += 1) {
      fake.failNext('connect', 'unreachable', "home-pc can't be reached.");
    }
    await openRoutines(fake, 'Unreachable');

    expect(await screen.findByText('Could not load routines from home-pc')).toBeTruthy();
    expect(screen.getByText('Work that was running there may still be running.', { exact: false })).toBeTruthy();
  });

  it('says a host without teams needs a newer MassOrca, and does not ask it for routines', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()], teams: [team({ id: 'team-1' })] });
    await openRoutines(fake);

    expect(await screen.findByText('Routines are not on This PC')).toBeTruthy();
    expect(screen.getByText('Routines need a newer MassOrca on This PC.')).toBeTruthy();
    expect(fake.calls).not.toContain('teams');
  });
});
