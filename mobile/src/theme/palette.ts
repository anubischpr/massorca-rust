import { useColorScheme } from 'react-native';

import { colorTokens, type ColorTokenName } from './tokens.generated';

export type Palette = Readonly<Record<ColorTokenName, string>>;

/** The colour tokens of the system theme (decision 0080: follows the system, no manual override yet). */
export function usePalette(): Palette {
  return useColorScheme() === 'dark' ? colorTokens.dark : colorTokens.light;
}
