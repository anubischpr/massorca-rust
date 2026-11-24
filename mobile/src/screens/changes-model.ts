import type { GitFileStatus, GitStatusSummary } from '../binding/types';
import { AHEAD_BEHIND, CHANGE_KIND_WORDS } from '../copy/changes';

export type ChangeKind = keyof typeof CHANGE_KIND_WORDS;

/** The change words of the binding (snake_case) and the kinds they stand for. */
const KIND_OF_WORD = new Map<string, ChangeKind>([
  ['added', 'added'],
  ['modified', 'modified'],
  ['deleted', 'deleted'],
  ['renamed', 'renamed'],
  ['copied', 'copied'],
  ['type_changed', 'typeChanged'],
]);

export interface ChangeRow {
  readonly path: string;
  readonly kind: ChangeKind;
  readonly name: string;
  /** The folders above the file, or undefined for a file at the top. */
  readonly directory: string | undefined;
}

/** The one change a row shows: a conflict first, then the working tree, then the index. */
export function changeKindOf(file: GitFileStatus): ChangeKind {
  if (file.conflict !== undefined) return 'conflict';
  const change = file.unstaged ?? file.staged;
  if (change !== undefined) return KIND_OF_WORD.get(change) ?? 'unknown';
  return file.untracked ? 'untracked' : 'unknown';
}

export function changeRows(files: readonly GitFileStatus[]): readonly ChangeRow[] {
  return files.map((file) => {
    const slash = file.path.lastIndexOf('/');
    return {
      path: file.path,
      kind: changeKindOf(file),
      name: slash < 0 ? file.path : file.path.slice(slash + 1),
      directory: slash < 0 ? undefined : file.path.slice(0, slash),
    };
  });
}

export function conflictCount(files: readonly GitFileStatus[]): number {
  return files.filter((file) => file.conflict !== undefined).length;
}

/** "2 ahead, 0 behind" when the branch has an upstream; undefined without one. */
export function aheadBehindLine(status: GitStatusSummary): string | undefined {
  if (status.ahead === undefined && status.behind === undefined) return undefined;
  return AHEAD_BEHIND(status.ahead ?? 0n, status.behind ?? 0n);
}
