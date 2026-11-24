import { NavigationContainer } from '@react-navigation/native';
import { createNativeStackNavigator } from '@react-navigation/native-stack';

import { CONFIRM_TITLE, PAIR_TITLE } from '../copy/pairing';
import { PROJECTS_TITLE } from '../copy/workspaces';
import { ConfirmPairingScreen } from '../screens/confirm-pairing-screen';
import { HomeScreen } from '../screens/home-screen';
import { PairScreen } from '../screens/pair-screen';
import { PlaceholderScreen } from '../screens/placeholder-screen';
import { ProtocolGateScreen } from '../screens/protocol-gate-screen';
import { WorkspacesScreen } from '../screens/workspaces-screen';
import type { RootStackParamList } from './routes';

const Stack = createNativeStackNavigator<RootStackParamList>();

/** The compact stack: one pane at a time, Home first, the system back gesture goes up one screen (M-SH-01). */
export function AppNavigator() {
  return (
    <NavigationContainer>
      <Stack.Navigator initialRouteName="Home">
        <Stack.Screen name="Home" component={HomeScreen} options={{ title: 'MassOrca' }} />
        <Stack.Screen name="Connect" component={PairScreen} options={{ title: PAIR_TITLE }} />
        <Stack.Screen name="ConfirmPairing" component={ConfirmPairingScreen} options={{ title: CONFIRM_TITLE }} />
        <Stack.Screen name="ProtocolGate" component={ProtocolGateScreen} options={{ title: 'Protocol check' }} />
        <Stack.Screen name="Workspaces" component={WorkspacesScreen} options={{ title: PROJECTS_TITLE }} />
        <Stack.Screen name="Terminal" options={{ title: 'Terminal' }}>
          {() => <PlaceholderScreen title="Terminal" />}
        </Stack.Screen>
        <Stack.Screen name="Changes" options={{ title: 'Changes' }}>
          {() => <PlaceholderScreen title="Changes" />}
        </Stack.Screen>
        <Stack.Screen name="ConnectionLog" options={{ title: 'Connection log' }}>
          {() => <PlaceholderScreen title="Connection log" />}
        </Stack.Screen>
      </Stack.Navigator>
    </NavigationContainer>
  );
}
