import React, { useEffect, useMemo, useRef, useState } from 'react';
import { BackHandler, StyleSheet, View } from 'react-native';
import type { DeviceProfile, User } from '@nuvora/core';
import { LiveLink, useLive } from './live';
import { NavContext, type Nav, type Route } from './nav';
import { Connect, SignIn } from './screens/Auth';
import { Home } from './screens/Home';
import { Library } from './screens/Library';
import { Movie } from './screens/Movie';
import { Player } from './screens/Player';
import { Search } from './screens/Search';
import { Settings } from './screens/Settings';
import { Show } from './screens/Show';
import { followParty, InviteDialog, Together } from './screens/Together';
import { clientFor, probeDevice } from './server';
import { AppContext, type AppState } from './state';
import { loadStorage, recentServers, savedSession, saveSession, type Session } from './storage';
import { colors } from './theme';
import { Loading, ScreenHost } from './ui';

/** A screen in the stack; `key` tells two visits to the same route apart. */
interface Entry {
  key: number;
  route: Route;
}

let keys = 0;
const entry = (route: Route): Entry => ({ key: ++keys, route });

/** Where to start: Home when signed in, else Sign in to the last server, else Connect. */
function firstRoute(session: Session | null): Route {
  if (session) return { name: 'home' };
  const last = recentServers()[0];
  return last ? { name: 'signIn', server: last } : { name: 'connect' };
}

export const App = () => {
  const [device, setDevice] = useState<DeviceProfile | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [stack, setStack] = useState<Entry[]>([]);
  const stackRef = useRef(stack);
  stackRef.current = stack;

  useEffect(() => {
    void Promise.all([probeDevice(), loadStorage()]).then(([probed]) => {
      const saved = savedSession();
      setDevice(probed);
      setSession(saved);
      setStack([entry(firstRoute(saved))]);
    });
  }, []);

  const nav = useMemo<Nav>(
    () => ({
      push: (route) => setStack((s) => [...s, entry(route)]),
      pop: () => setStack((s) => (s.length < 2 ? s : s.slice(0, -1))),
      replace: (route) => setStack((s) => [...s.slice(0, -1), entry(route)]),
      // Sections replace each other above Home, so Back from any of them returns Home.
      section: (route) =>
        setStack((s) => {
          const home = s[0]?.route.name === 'home' ? s[0] : entry({ name: 'home' });
          return route.name === 'home' ? [home] : [home, entry(route)];
        }),
      reset: (route) => setStack([entry(route)]),
    }),
    [],
  );

  // Back goes back a screen; on the first screen it leaves the app. Screens that
  // handle Back themselves (menus, the player) register after this, so they hear it first.
  useEffect(() => {
    const sub = BackHandler.addEventListener('hardwareBackPress', () => {
      if (stackRef.current.length < 2) return false;
      nav.pop();
      return true;
    });
    return () => sub.remove();
  }, [nav]);

  // Watch Together's live connection, while signed in.
  const serverUrl = session?.server.url;
  const token = session?.token;
  const live = useMemo(() => (serverUrl && token ? new LiveLink(serverUrl, token) : null), [serverUrl, token]);
  useEffect(() => {
    if (!live) return;
    live.start();
    // Someone in the party started something: everyone goes to it.
    const off = live.onNotice((notice) => {
      const playback = notice.party.playback;
      if (!playback) return;
      if (live.playing?.itemId === playback.item_id && live.playing.fileId === playback.file_id) return;
      followParty(nav, live, notice.party, stackRef.current[stackRef.current.length - 1]?.route.name === 'player');
    });
    return () => {
      off();
      live.stop();
    };
  }, [live, nav]);
  const party = useLive(live);
  const [answered, setAnswered] = useState<string[]>([]);
  const newInvite = party.invites.filter((i) => !answered.includes(`${i.party_id}:${i.sent_at_ms}`)).pop();

  const app = useMemo<AppState | null>(() => {
    if (!device) return null;
    const signOut = (options?: { changeServer?: boolean }) => {
      const server = session?.server;
      setSession(null);
      saveSession(null);
      setStack([entry(server && !options?.changeServer ? { name: 'signIn', server } : { name: 'connect' })]);
    };
    return {
      device,
      session,
      // A session the server ended (401) signs out here too.
      client: session ? clientFor(session.server.url, device, session.token, signOut) : null,
      live,
      signedIn: (next) => {
        setSession(next);
        saveSession(next);
      },
      signOut,
      updateUser: (user: User) =>
        setSession((s) => {
          const next = s && { ...s, user };
          saveSession(next);
          return next;
        }),
      onlyH264: () => {
        const video = device.video.filter((v) => v.codec === 'h264');
        if (video.length === device.video.length) return false;
        setDevice({ ...device, video });
        return true;
      },
    };
  }, [device, session, live]);

  if (!app || !stack.length) return <Loading />;
  const top = stack[stack.length - 1]!;
  return (
    <AppContext.Provider value={app}>
      <NavContext.Provider value={nav}>
        <View style={styles.app}>
          {stack.map((e) => (
            <ScreenHost key={e.key} active={e === top}>
              <Screen route={e.route} />
            </ScreenHost>
          ))}
          {newInvite && app.session && (
            <InviteDialog
              invite={newInvite}
              playerOnTop={top.route.name === 'player'}
              onClose={() => setAnswered((a) => [...a, `${newInvite.party_id}:${newInvite.sent_at_ms}`])}
            />
          )}
        </View>
      </NavContext.Provider>
    </AppContext.Provider>
  );
};

function Screen({ route }: { route: Route }) {
  switch (route.name) {
    case 'connect':
      return <Connect />;
    case 'signIn':
      return <SignIn server={route.server} />;
    case 'home':
      return <Home />;
    case 'search':
      return <Search />;
    case 'together':
      return <Together />;
    case 'settings':
      return <Settings />;
    case 'library':
      return <Library id={route.id} title={route.title} />;
    case 'movie':
      return <Movie id={route.id} />;
    case 'show':
      return <Show id={route.id} />;
    case 'player':
      return <Player itemId={route.itemId} {...(route.fileId !== undefined && { fileId: route.fileId })} {...(route.startMs !== undefined && { startMs: route.startMs })} />;
  }
}

const styles = StyleSheet.create({
  app: { flex: 1, backgroundColor: colors.canvas },
});
