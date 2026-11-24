import type { Watch } from './types';

/**
 * Turns a pull-based watch into an async iterator. Leaving the loop early (break, return or a throw in the body)
 * or an error from `next` closes the watch, so the host connection is not left open.
 */
export async function* asyncIterate<E>(watch: Watch<E>): AsyncGenerator<E, void, undefined> {
  try {
    for (;;) {
      const event = await watch.next();
      if (event === null) return;
      yield event;
    }
  } finally {
    watch.close();
  }
}
