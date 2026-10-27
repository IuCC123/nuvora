import { createContext, useContext } from 'react';
import type { Server } from './server';

/** Every screen, with what it needs to open. */
export type Route =
  | { name: 'connect' }
  | { name: 'signIn'; server: Server }
  | { name: 'home' }
  | { name: 'search' }
  | { name: 'together' }
  | { name: 'settings' }
  | { name: 'library'; id: number; title: string }
  | { name: 'movie'; id: number }
  | { name: 'show'; id: number }
  /** `fileId`: the version (left out, the one being resumed, else the best); `startMs`: where (left out, where the person stopped). */
  | { name: 'player'; itemId: number; fileId?: number; startMs?: number };

/** The main sections, which replace each other above Home. */
export type SectionRoute = Extract<Route, { name: 'home' | 'search' | 'together' | 'settings' | 'library' }>;

export interface Nav {
  push(route: Route): void;
  /** Back one screen, if there is one. */
  pop(): void;
  /** In place of the screen on top: the next episode, or what the party moved on to. */
  replace(route: Route): void;
  /** A section of the side menu, straight above Home, so Back from it goes Home. */
  section(route: SectionRoute): void;
  /** A fresh start at `route`: after signing in or out. */
  reset(route: Route): void;
}

export const NavContext = createContext<Nav | null>(null);

export function useNav(): Nav {
  const nav = useContext(NavContext);
  if (!nav) throw new Error('useNav outside the app');
  return nav;
}

/** Where a poster or result goes: a movie or show page, or (Continue Watching) straight back into an episode. */
export function open(nav: Nav, card: { kind: string; id: number }) {
  if (card.kind === 'movie') nav.push({ name: 'movie', id: card.id });
  else if (card.kind === 'show') nav.push({ name: 'show', id: card.id });
  else if (card.kind === 'episode') nav.push({ name: 'player', itemId: card.id });
}
