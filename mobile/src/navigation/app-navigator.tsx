import { NavigationContainer } from '@react-navigation/native';
import { createNativeStackNavigator } from '@react-navigation/native-stack';

import { HomeScreen } from '../screens/home-screen';
import { PlaceholderScreen } from '../screens/placeholder-screen';
import type { RootStackParamList } from './routes';

const Stack = createNativeStackNavigator<RootStackParamList>();

/** The compact stack: one pane at a time, Home first, the system back gesture goes up one screen (M-SH-01). */
export function AppNavigator() {
  return (
    <NavigationContainer>
      <Stack.Navigator initialRouteName="Home">
        <Stack.Screen name="Home" component={HomeScreen} options={{ title: 'MassOrca' }} />
        <Stack.Screen name="Connect" options={{ title: 'Pair desktop' }}>
          {() => <PlaceholderScreen title="Pair desktop" />}
        </Stack.Screen>
        <Stack.Screen name="ProtocolGate" options={{ title: 'Protocol check' }}>
          {() => <PlaceholderScreen title="Protocol check" />}
        </Stack.Screen>
        <Stack.Screen name="Workspaces" options={{ title: 'Workspaces' }}>
          {() => <PlaceholderScreen title="Workspaces" />}
        </Stack.Screen>
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
