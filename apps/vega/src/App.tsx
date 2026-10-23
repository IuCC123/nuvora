import React, { useEffect, useMemo, useState } from 'react';
import { BackHandler, StyleSheet, View } from 'react-native';
import type { DeviceProfile } from '@nuvora/core';
import { NavContext, type Nav, type Route } from './nav';
import { Connect, SignIn } from './screens/Auth';
import { Home } from './screens/Home';
import { Movie, Show } from './screens/Item';
import { Player } from './screens/Player';
import { clientFor, probeDevice } from './server';
import { lastServer, loadSession, saveSession, type Session } from './session';
import { AppContext, type AppState } from './state';
import { colors } from './theme';
import { Loading } from './ui';

/** Where to start: Home when signed in, else Sign in to the last server, else Connect. */
async function start(): Promise<{ device: DeviceProfile; session: Session | null; route: Route }> {
  const [device, session, last] = await Promise.all([probeDevice(), loadSession(), lastServer()]);
  const route: Route = session ? { name: 'home' } : last ? { name: 'signIn', server: last } : { name: 'connect' };
  return { device, session, route };
}

export const App = () => {
  const [device, setDevice] = useState<DeviceProfile | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [stack, setStack] = useState<Route[]>([]);

  useEffect(() => {
    void start().then((s) => {
      setDevice(s.device);
      setSession(s.session);
      setStack([s.route]);
    });
  }, []);

  const nav = useMemo<Nav>(
    () => ({
      push: (route) => setStack((s) => [...s, route]),
      pop: () => setStack((s) => (s.length < 2 ? s : s.slice(0, -1))),
      reset: (route) => setStack([route]),
    }),
    [],
  );

  // Back goes back a screen; on the first screen it leaves the app.
  const depth = stack.length;
  useEffect(() => {
    const subscription = BackHandler.addEventListener('hardwareBackPress', () => {
      if (depth < 2) return false;
      nav.pop();
      return true;
    });
    return () => subscription.remove();
  }, [depth, nav]);

  const app = useMemo<AppState | null>(() => {
    if (!device) return null;
    const signOut = () => {
      const server = session?.server;
      setSession(null);
      void saveSession(null);
      setStack([server ? { name: 'signIn', server } : { name: 'connect' }]);
    };
    return {
      device,
      session,
      // A session the server ended (401) signs out here too.
      client: session ? clientFor(session.server.url, device, session.token, signOut) : null,
      signedIn: (next) => {
        setSession(next);
        void saveSession(next);
      },
      signOut,
      onlyH264: () => {
        const video = device.video.filter((v) => v.codec === 'h264');
        if (video.length === device.video.length) return false;
        setDevice({ ...device, video });
        return true;
      },
    };
  }, [device, session]);

  const route = stack[stack.length - 1];
  if (!app || !route) return <Loading />;
  return (
    <AppContext.Provider value={app}>
      <NavContext.Provider value={nav}>
        <View style={styles.app}>
          <Screen route={route} />
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
    case 'movie':
      return <Movie id={route.id} />;
    case 'show':
      return <Show id={route.id} />;
    case 'player':
      return <Player itemId={route.itemId} {...(route.startMs !== undefined && { startMs: route.startMs })} />;
  }
}

const styles = StyleSheet.create({
  app: { flex: 1, backgroundColor: colors.canvas },
});
