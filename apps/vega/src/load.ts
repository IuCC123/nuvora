import { useCallback, useEffect, useRef, useState } from 'react';
import { describeError } from '@nuvora/core';
import { useScreenActive } from './ui';

export type Load<T> = { kind: 'loading' } | { kind: 'ready'; value: T } | { kind: 'failed'; error: string };

/**
 * Loads `fetch` when `key` changes; `reload` fetches again. With `refresh`,
 * it also fetches again whenever the screen comes back to the top (back from
 * the player, positions have moved), keeping what's shown until the answer
 * arrives.
 */
export function useLoad<T>(key: unknown, fetch: () => Promise<T>, options?: { refresh?: boolean }): [Load<T>, () => void] {
  const [state, setState] = useState<Load<T>>({ kind: 'loading' });
  const latest = useRef(fetch);
  latest.current = fetch;
  const generation = useRef(0);

  const run = useCallback((quiet: boolean) => {
    const mine = ++generation.current;
    if (!quiet) setState({ kind: 'loading' });
    latest.current().then(
      (value) => mine === generation.current && setState({ kind: 'ready', value }),
      (e: unknown) => mine === generation.current && !quiet && setState({ kind: 'failed', error: describeError(e) }),
    );
  }, []);

  useEffect(() => run(false), [key, run]);

  const active = useScreenActive();
  const wasActive = useRef(active);
  useEffect(() => {
    if (options?.refresh && active && !wasActive.current) run(true);
    wasActive.current = active;
  }, [active, options?.refresh, run]);

  return [state, useCallback(() => run(false), [run])];
}
