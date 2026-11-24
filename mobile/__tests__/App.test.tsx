/**
 * @format
 */

import { render, screen } from '@testing-library/react-native';

import App from '../App';
import { FakeMassOrca } from '../src/binding/fake';

// The safe-area provider renders its children only after the native layer reports insets; the library's mock does that here.
jest.mock('react-native-safe-area-context', () => require('react-native-safe-area-context/jest/mock').default);

// The first render of the navigator loads the whole stack; a cold run on a loaded machine needs more than the default.
jest.setTimeout(20_000);

describe('App', () => {
  it('mounts on Home with no host paired and shows the connect hint', async () => {
    await render(<App binding={new FakeMassOrca()} />);

    expect(await screen.findByText('Connect your desktop')).toBeTruthy();
  });
});
