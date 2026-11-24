import { fireEvent, render, screen } from '@testing-library/react-native';
import { Linking } from 'react-native';

import App from '../../App';
import { FakeMassOrca } from '../binding/fake';
import { hostSummary, ProtocolRefusingFake } from '../state/test-fixtures';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

async function openGate(fake: FakeMassOrca | ProtocolRefusingFake) {
  await render(<App binding={fake} />);
  await fireEvent.press(await screen.findByRole('button', { name: 'home-pc, Update needed' }));
}

describe('protocol gate', () => {
  it('says this PC needs an update when it speaks only versions older than this app', async () => {
    await openGate(new ProtocolRefusingFake({ min: 0, max: 0 }));

    expect(await screen.findByText('home-pc needs an update')).toBeTruthy();
    expect(
      screen.getByText(
        'MassOrca on home-pc speaks protocol 0. This app needs protocol 1 or newer. Update MassOrca on home-pc, then check again.',
      ),
    ).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Check again' })).toBeTruthy();
  });

  it('sends the phone to Google Play when this app is the one that is out of date', async () => {
    const openURL = jest.spyOn(Linking, 'openURL').mockResolvedValue(undefined);
    await openGate(new ProtocolRefusingFake({ min: 2, max: 3 }));

    expect(await screen.findByText('This app needs an update')).toBeTruthy();
    expect(screen.getByText('home-pc speaks protocol 2. Update MassOrca from Google Play, then open it again.')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Open Google Play' }));

    expect(openURL).toHaveBeenCalledWith('market://details?id=dev.massorca.app');
    openURL.mockRestore();
  });

  it('says the versions do not match when the host does not say which range it speaks', async () => {
    const fake = new FakeMassOrca({ hosts: [hostSummary()] });
    fake.failNext('connect', 'protocol_mismatch', 'The service refused the connection.');
    await openGate(fake);

    expect(await screen.findByText('The two versions do not match')).toBeTruthy();
    expect(
      screen.getByText('This app and home-pc speak different protocol versions. Update the older MassOrca, then check again.'),
    ).toBeTruthy();
  });

  it('checks again and opens the projects once the protocol agrees', async () => {
    await openGate(new ProtocolRefusingFake({ min: 0, max: 0 }, 1));
    expect(await screen.findByText('home-pc needs an update')).toBeTruthy();

    await fireEvent.press(screen.getByRole('button', { name: 'Check again' }));

    expect(await screen.findByText(/^(Projects|No workspaces yet)$/)).toBeTruthy();
  });
});
