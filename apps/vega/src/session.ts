import AsyncStorage from '@amazon-devices/react-native-async-storage__async-storage';
import type { User } from '@nuvora/core';
import type { Server } from './server';

/** Signed in to one server, kept across restarts. */
export interface Session {
  server: Server;
  token: string;
  user: User;
}

const SESSION = 'nuvora.session';
const LAST_SERVER = 'nuvora.server';

const read = async <T,>(key: string): Promise<T | null> => {
  try {
    const text = await AsyncStorage.getItem(key);
    return text ? (JSON.parse(text) as T) : null;
  } catch {
    return null;
  }
};

export const loadSession = () => read<Session>(SESSION);
/** The server last connected to, offered again after signing out. */
export const lastServer = () => read<Server>(LAST_SERVER);

export async function saveSession(session: Session | null) {
  if (session) {
    await AsyncStorage.multiSet([
      [SESSION, JSON.stringify(session)],
      [LAST_SERVER, JSON.stringify(session.server)],
    ]);
  } else {
    await AsyncStorage.removeItem(SESSION);
  }
}
