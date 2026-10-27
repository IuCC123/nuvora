import AsyncStorage from '@amazon-devices/react-native-async-storage__async-storage';
import type { RememberedSubtitle, User } from '@nuvora/core';
import type { Server } from './server';

/** Signed in to one server, kept across restarts. */
export interface Session {
  server: Server;
  token: string;
  user: User;
}

/** How subtitles look on this TV. */
export interface SubtitleStyle {
  size: 'small' | 'medium' | 'large';
  background: 'none' | 'shadow' | 'box';
}

export const SUBTITLE_SIZES = [
  ['small', 'Small'],
  ['medium', 'Medium'],
  ['large', 'Large'],
] as const;
export const SUBTITLE_BACKGROUNDS = [
  ['none', 'None'],
  ['shadow', 'Shadow'],
  ['box', 'Box'],
] as const;

interface Stored {
  session: Session | null;
  /** Newest first, at most four. */
  servers: Server[];
  subtitleStyle: SubtitleStyle;
  /** The last subtitle choice per user and title: `"3|show:12"`. */
  subtitleChoices: Record<string, RememberedSubtitle>;
}

const KEY = {
  session: 'nuvora.session',
  servers: 'nuvora.servers',
  subtitleStyle: 'nuvora.subtitle-style',
  subtitleChoices: 'nuvora.subtitle-choices',
  /** The one server kept before the list. */
  legacyServer: 'nuvora.server',
} as const;

const stored: Stored = {
  session: null,
  servers: [],
  subtitleStyle: { size: 'medium', background: 'shadow' },
  subtitleChoices: {},
};

const parse = <T,>(text: string | null | undefined): T | null => {
  try {
    return text ? (JSON.parse(text) as T) : null;
  } catch {
    return null;
  }
};

const write = (key: string, value: unknown) => {
  void AsyncStorage.setItem(key, JSON.stringify(value)).catch(() => undefined);
};

/** Reads everything once, at start; after that the getters answer at once. */
export async function loadStorage(): Promise<void> {
  try {
    const pairs = await AsyncStorage.multiGet(Object.values(KEY));
    const get = (key: string) => pairs.find(([k]) => k === key)?.[1];
    stored.session = parse<Session>(get(KEY.session));
    const legacy = parse<Server>(get(KEY.legacyServer));
    stored.servers = parse<Server[]>(get(KEY.servers)) ?? (legacy ? [legacy] : []);
    stored.subtitleStyle = { ...stored.subtitleStyle, ...parse<SubtitleStyle>(get(KEY.subtitleStyle)) };
    stored.subtitleChoices = parse<Record<string, RememberedSubtitle>>(get(KEY.subtitleChoices)) ?? {};
  } catch {
    // A first start, or storage that can't be read: start empty.
  }
}

export const savedSession = () => stored.session;

/** Saves the session; signing in also puts its server first among the recent ones. */
export function saveSession(session: Session | null) {
  stored.session = session;
  if (session) {
    write(KEY.session, session);
    stored.servers = [session.server, ...stored.servers.filter((s) => s.url !== session.server.url && (!s.id || s.id !== session.server.id))].slice(0, 4);
    write(KEY.servers, stored.servers);
  } else {
    void AsyncStorage.removeItem(KEY.session).catch(() => undefined);
  }
}

export const recentServers = () => stored.servers;

export const subtitleStyle = () => stored.subtitleStyle;
export function saveSubtitleStyle(style: SubtitleStyle) {
  stored.subtitleStyle = style;
  write(KEY.subtitleStyle, style);
}

export const subtitleChoice = (userId: number, scope: string) => stored.subtitleChoices[`${userId}|${scope}`] ?? null;
export function saveSubtitleChoice(userId: number, scope: string, choice: RememberedSubtitle) {
  stored.subtitleChoices = { ...stored.subtitleChoices, [`${userId}|${scope}`]: choice };
  write(KEY.subtitleChoices, stored.subtitleChoices);
}
