import React, { useEffect, useRef, useState, type ReactNode } from 'react';
import { StyleSheet, Text, TextInput, View } from 'react-native';
import { ApiError, describeError } from '@nuvora/core';
import { discoverServers } from '../discovery';
import { Icon } from '../icons';
import { useNav } from '../nav';
import { clientFor, findServer, type Server } from '../server';
import { useApp } from '../state';
import { recentServers } from '../storage';
import { colors, type } from '../theme';
import { Button, Field, FocusGuide, Glow } from '../ui';

const bare = (url: string) => url.replace(/^https?:\/\//, '');

/** The wordmark on the left, the form on the right, over a warm wash. */
function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <View style={styles.screen}>
      <Glow color="#2A2410" cx={0.12} cy={0} r={0.9} />
      <View style={styles.brand}>
        <View style={styles.wordmark}>
          <Icon name="play" size={48} color={colors.accent} />
          <Text style={styles.name}>nuvora</Text>
        </View>
        <Text style={styles.tagline}>Your movies and shows, on the big screen.</Text>
      </View>
      <FocusGuide autoFocus style={styles.form}>
        {children}
      </FocusGuide>
    </View>
  );
}

/** The first screen: which Nuvora server to use, from the network, the recent ones, or typed. */
export function Connect() {
  const { device } = useApp();
  const nav = useNav();
  const [address, setAddress] = useState('');
  const [nearby, setNearby] = useState<Server[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const recent = recentServers().filter((saved) => !nearby.some((s) => s.url === saved.url || (s.id && s.id === saved.id)));

  useEffect(() => {
    const signal = { aborted: false };
    void discoverServers((server) => setNearby((list) => (list.some((s) => s.url === server.url || (s.id && s.id === server.id)) ? list : [...list, server])), signal);
    return () => {
      signal.aborted = true;
    };
  }, []);

  const connect = async (typed: string) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      nav.push({ name: 'signIn', server: await findServer(typed, device) });
    } catch (e) {
      setError(e instanceof Error ? e.message : "Couldn't connect.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <AuthLayout>
      <Text style={styles.title}>Connect to your server</Text>
      <Text style={styles.lead}>
        {nearby.length
          ? 'Choose your server, or type the address of another.'
          : "Type the address of your Nuvora server: the installer prints it, and it's in your browser's address bar, for example 192.168.1.20:8640."}
      </Text>
      {nearby.length > 0 && (
        <>
          <Text style={styles.kicker}>ON YOUR NETWORK</Text>
          <View style={styles.buttons}>
            {nearby.slice(0, 3).map((server, i) => (
              <Button key={server.url} label={`${server.name} · ${bare(server.url)}`} icon="server" preferred={i === 0} disabled={busy} onPress={() => void connect(server.url)} />
            ))}
          </View>
          <View style={styles.gap28} />
        </>
      )}
      <Field label="Server address" value={address} onChangeText={setAddress} onSubmit={() => void connect(address)} placeholder="192.168.1.20:8640" />
      <View style={[styles.buttons, styles.gapTop18]}>
        <Button label={busy ? 'Connecting…' : 'Connect'} primary preferred={!nearby.length} disabled={busy} onPress={() => void connect(address)} />
        {error && <Text style={styles.error}>{error}</Text>}
      </View>
      {recent.length > 0 && (
        <>
          <Text style={[styles.kicker, styles.gapTop40]}>RECENT</Text>
          <View style={styles.buttons}>
            {recent.slice(0, 4).map((server) => (
              <Button key={server.url} label={`${server.name} · ${bare(server.url)}`} icon="server" onPress={() => void connect(server.url)} />
            ))}
          </View>
        </>
      )}
    </AuthLayout>
  );
}

/** Signing in to the server just found or remembered. */
export function SignIn({ server }: { server: Server }) {
  const { device, signedIn } = useApp();
  const nav = useNav();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const passwordField = useRef<TextInput>(null);

  const signIn = async () => {
    if (busy) return;
    if (!username.trim() || !password) {
      setError('Enter your username and password.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const session = await clientFor(server.url, device).appSignIn({ username: username.trim(), password });
      signedIn({ server, token: session.token, user: session.user });
      nav.reset({ name: 'home' });
    } catch (e) {
      setError(e instanceof ApiError && e.status === 401 ? `That username and password don't match an account on ${server.name}.` : describeError(e));
      setBusy(false);
    }
  };

  return (
    <AuthLayout>
      <Text style={styles.title}>Sign in</Text>
      <Text style={styles.lead}>
        to {server.name} at {bare(server.url)}
      </Text>
      {/* Done on the username goes on to the password, whose Done signs in. */}
      <Field label="Username" value={username} onChangeText={setUsername} onSubmit={() => passwordField.current?.focus()} />
      <View style={styles.gap14} />
      <Field ref={passwordField} label="Password" value={password} onChangeText={setPassword} onSubmit={() => void signIn()} secure />
      <View style={[styles.buttons, styles.gapTop18]}>
        <Button label={busy ? 'Signing in…' : 'Sign in'} primary preferred disabled={busy} onPress={() => void signIn()} />
        <Button label="Use another server" onPress={() => nav.reset({ name: 'connect' })} />
      </View>
      {error && <Text style={[styles.error, styles.gapTop14]}>{error}</Text>}
    </AuthLayout>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, flexDirection: 'row', alignItems: 'center', paddingHorizontal: 96, backgroundColor: colors.canvas },
  brand: { flex: 0.8 },
  wordmark: { flexDirection: 'row', alignItems: 'center', gap: 10 },
  name: { fontSize: 56, lineHeight: 64, fontWeight: '700', color: colors.text },
  tagline: { ...type.titleMedium, color: colors.secondary, marginTop: 10, maxWidth: 280 },
  form: { flex: 1.2 },
  title: { ...type.display, color: colors.text },
  lead: { ...type.bodyLarge, color: colors.secondary, marginTop: 12, marginBottom: 28 },
  kicker: { ...type.labelSmall, color: colors.muted, marginBottom: 12 },
  buttons: { flexDirection: 'row', flexWrap: 'wrap', alignItems: 'center', gap: 14 },
  error: { ...type.bodyMedium, color: colors.danger, flex: 1 },
  gap14: { height: 14 },
  gap28: { height: 28 },
  gapTop14: { marginTop: 14 },
  gapTop18: { marginTop: 18 },
  gapTop40: { marginTop: 40 },
});
