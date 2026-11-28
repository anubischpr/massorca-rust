import type { Host, RoutineRunSummary } from '../binding/types';
import type { RoutineSection } from './routines-model';

/** The capability that offers the team methods (`teams/1`); a host without it has no routines to read. */
export const TEAMS_CAPABILITY = 'teams/1';

export type RoutinesLoad =
  | { readonly status: 'ready'; readonly sections: readonly RoutineSection[] }
  | { readonly status: 'unsupported' }
  | { readonly status: 'error'; readonly message: string };

/**
 * Reads the routines of every team on the host, with each team's bots and each routine's newest run. A host that does
 * not offer teams answers `unsupported` without a call; teams without routines are left out.
 */
export async function loadRoutines(host: Host): Promise<RoutinesLoad> {
  if (!host.protocol().capabilities.includes(TEAMS_CAPABILITY)) return { status: 'unsupported' };
  try {
    const teams = await host.teams();
    const sections = await Promise.all(
      teams.map(async (team): Promise<RoutineSection> => {
        const [bots, routines] = await Promise.all([host.teamBots(team.id), host.teamRoutines(team.id)]);
        const runs = await Promise.all(
          routines.map((routine) => host.routineRuns(team.id, routine.id, 1)),
        );
        return {
          teamId: team.id,
          teamName: team.name,
          routines: routines.map((routine, index) => {
            const bot = bots.find((candidate) => candidate.id === routine.botId);
            const newest: RoutineRunSummary | undefined = runs[index]?.[0];
            return { routine, botName: bot?.name, lastRun: newest };
          }),
        };
      }),
    );
    return { status: 'ready', sections: sections.filter((section) => section.routines.length > 0) };
  } catch (error) {
    return { status: 'error', message: error instanceof Error ? error.message : String(error) };
  }
}
