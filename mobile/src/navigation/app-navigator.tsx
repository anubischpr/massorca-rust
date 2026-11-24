import { NavigationContainer } from '@react-navigation/native';
import { createNativeStackNavigator } from '@react-navigation/native-stack';

import { CONNECTION_LOG_TITLE } from '../copy/connection';
import { TERMINAL_TITLE } from '../copy/terminal';
import { PROJECTS_TITLE, WORKSPACE_TITLE } from '../copy/workspaces';
import { CONFIRM_TITLE, PAIR_TITLE } from '../copy/pairing';
import { ConfirmPairingScreen } from '../screens/confirm-pairing-screen';
import { ConnectionLogScreen } from '../screens/connection-log-screen';
import { HomeScreen } from '../screens/home-screen';
import { PairScreen } from '../screens/pair-screen';
import { ProtocolGateScreen } from '../screens/protocol-gate-screen';
import { TerminalScreen } from '../screens/terminal-screen';
import { WorkspaceScreen } from '../screens/workspace-screen';
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
        <Stack.Screen name="Terminal" options={{ title: TERMINAL_TITLE }}>
          {({ route, navigation }) => (
            <TerminalScreen
              host={route.params.host}
              terminal={route.params.terminal}
              workspace={route.params.workspace}
              onOpenDetails={() => navigation.navigate('ConnectionLog', { host: route.params.host })}
            />
          )}
        </Stack.Screen>
        <Stack.Screen name="Workspace" options={{ title: WORKSPACE_TITLE }}>
          {({ route, navigation }) => (
            <WorkspaceScreen
              host={route.params.host}
              workspace={route.params.workspace}
              noGit={route.params.noGit}
              onOpenDetails={() => navigation.navigate('ConnectionLog', { host: route.params.host })}
              onOpenTerminal={(terminal) =>
                navigation.navigate('Terminal', { host: route.params.host, terminal, workspace: route.params.workspace })
              }
            />
          )}
        </Stack.Screen>
        <Stack.Screen name="ConnectionLog" component={ConnectionLogScreen} options={{ title: CONNECTION_LOG_TITLE }} />
      </Stack.Navigator>
    </NavigationContainer>
  );
}
