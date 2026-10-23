import React, { useEffect, useRef, useState, type ReactNode } from 'react';
import { StyleSheet, Text, TextInput, View } from 'react-native';
import { describeError } from '@nuvora/core';
import { useNav } from '../nav';
import { clientFor, findServer, type Server } from '../server';
import { lastServer } from '../session';
import { useApp } from '../state';
import { colors, gutter, type } from '../theme';
import { Button, Field } from '../ui';

/** The wordmark on the left, the form on the right, as on the Fire TV app. */
function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <View style={styles.screen}>
      <View style={styles.brand}>
        <View style={styles.wordmark}>
          <View style={styles.mark} />
          <Text style={styles.name}>nuvora</Text>
        </View>
        <Text style={styles.tagline}>Your movies and shows, on the big screen.</Text>
      </View>
      <View style={styles.form}>{children}</View>
    </View>
  );
}

export function Connect() {
  const { device } = useApp();
  const nav = useNav();
  const [address, setAddress] = useState('');
  const [recent, setRecent] = useState<Server | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void lastServer().then(setRecent);
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
      <Text style={styles.lead}>Type the address of your Nuvora server: it's in your browser's address bar, for example 192.168.1.20:8640.</Text>
      <Field label="Server address" value={address} onChangeText={setAddress} onSubmit={() => connect(address)} placeholder="192.168.1.20:8640" />
      <View style={styles.actions}>
        <Button label={busy ? 'Connecting…' : 'Connect'} onPress={() => connect(address)} primary preferred={!recent} />
        {recent && <Button label={`${recent.name} · ${recent.url.replace(/^https?:\/\//, '')}`} onPress={() => connect(recent.url)} preferred />}
      </View>
      {error && <Text style={styles.error}>{error}</Text>}
    </AuthLayout>
  );
}

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
      setError(describeError(e, { bad_credentials: `That username and password don't match an account on ${server.name}.` }));
      setBusy(false);
    }
  };

  return (
    <AuthLayout>
      <Text style={styles.title}>Sign in</Text>
      <Text style={styles.lead}>
        to {server.name} at {server.url.replace(/^https?:\/\//, '')}
      </Text>
      {/* Done on the username goes on to the password, whose Done signs in. */}
      <Field label="Username" value={username} onChangeText={setUsername} onSubmit={() => passwordField.current?.focus()} />
      <Field ref={passwordField} label="Password" value={password} onChangeText={setPassword} onSubmit={signIn} secure />
      <View style={styles.actions}>
        <Button label={busy ? 'Signing in…' : 'Sign in'} onPress={signIn} primary preferred />
        <Button label="Use another server" onPress={() => nav.reset({ name: 'connect' })} />
      </View>
      {error && <Text style={styles.error}>{error}</Text>}
    </AuthLayout>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, flexDirection: 'row', alignItems: 'center', paddingHorizontal: gutter + 24, backgroundColor: colors.canvas },
  brand: { flex: 0.8 },
  wordmark: { flexDirection: 'row', alignItems: 'center' },
  // The play mark: a triangle drawn with borders.
  mark: {
    width: 0,
    height: 0,
    borderTopWidth: 15,
    borderBottomWidth: 15,
    borderLeftWidth: 24,
    borderTopColor: 'transparent',
    borderBottomColor: 'transparent',
    borderLeftColor: colors.accent,
    marginRight: 10,
  },
  name: { fontSize: 44, fontWeight: '800', color: colors.text },
  tagline: { ...type.bodyLarge, color: colors.secondary, marginTop: 6, maxWidth: 260 },
  form: { flex: 1.2 },
  title: { ...type.headline, color: colors.text },
  lead: { ...type.body, color: colors.secondary, marginTop: 8, marginBottom: 22 },
  actions: { flexDirection: 'row', gap: 12, marginTop: 6 },
  error: { ...type.body, color: colors.danger, marginTop: 14 },
});
