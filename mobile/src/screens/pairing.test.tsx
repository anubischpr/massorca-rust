import { fireEvent, render, screen } from '@testing-library/react-native';

import App from '../../App';
import { FakeMassOrca } from '../binding/fake';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

async function openPairing(fake: FakeMassOrca) {
  await render(<App binding={fake} />);
  await fireEvent.press(screen.getByRole('button', { name: 'Paste a code' }));
  return screen.findByLabelText('Pairing code');
}

describe('pairing from the first screen', () => {
  it('pastes a code, confirms the desktop, and pairs it', async () => {
    const fake = new FakeMassOrca();
    const field = await openPairing(fake);

    await fireEvent.changeText(field, 'fake-offer:example');
    await fireEvent.press(screen.getByRole('button', { name: 'Connect' }));

    expect(await screen.findByText('Pair with this desktop?')).toBeTruthy();
    expect(screen.getByLabelText('Name on this phone').props.value).toBe('vps');

    await fireEvent.press(screen.getByRole('button', { name: 'Pair' }));

    expect(await screen.findByText('Home')).toBeTruthy();
    expect(fake.hosts().map((host) => host.name)).toEqual(['vps']);
  });

  it('refuses a code that is not an offer and keeps Connect disabled until it changes', async () => {
    const field = await openPairing(new FakeMassOrca());

    await fireEvent.changeText(field, 'not a code');
    await fireEvent.press(screen.getByRole('button', { name: 'Connect' }));

    expect(
      await screen.findByText('Not a valid pairing code. Copy it from your computer and paste it again.'),
    ).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Connect' }).props.accessibilityState).toMatchObject({ disabled: true });
  });

  it('keeps Pair disabled under a name the host store refuses', async () => {
    const field = await openPairing(new FakeMassOrca());
    await fireEvent.changeText(field, 'fake-offer:example');
    await fireEvent.press(screen.getByRole('button', { name: 'Connect' }));
    await screen.findByText('Pair with this desktop?');

    await fireEvent.changeText(screen.getByLabelText('Name on this phone'), 'Bad Name');

    expect(screen.getByText('Use 1 to 32 letters, digits, - and _, starting with a letter or digit.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Pair' }).props.accessibilityState).toMatchObject({ disabled: true });
  });

  it('shows a failed pairing and retries it', async () => {
    const fake = new FakeMassOrca();
    fake.failNext('pair', 'unreachable', "vps can't be reached.");
    const field = await openPairing(fake);
    await fireEvent.changeText(field, 'fake-offer:example');
    await fireEvent.press(screen.getByRole('button', { name: 'Connect' }));
    await screen.findByText('Pair with this desktop?');

    await fireEvent.press(screen.getByRole('button', { name: 'Pair' }));

    expect(
      await screen.findByText("Could not pair: vps can't be reached. Check that both are on the same network."),
    ).toBeTruthy();
    await fireEvent.press(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByText('Home')).toBeTruthy();
    expect(fake.hosts()).toHaveLength(1);
  });

  it('says scanning comes later and offers the paste path instead', async () => {
    await render(<App binding={new FakeMassOrca()} />);
    await fireEvent.press(screen.getByRole('button', { name: 'Pair desktop' }));

    expect(await screen.findByText('Scanning comes later. Paste the pairing code from your computer for now.')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Paste code instead' }));
    expect(await screen.findByLabelText('Pairing code')).toBeTruthy();
  });
});
