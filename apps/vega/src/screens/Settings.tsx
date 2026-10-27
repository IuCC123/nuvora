import React, { useEffect, useState } from 'react';
import { BackHandler, ScrollView, StyleSheet, Text, View } from 'react-native';
import { describeError, languageName, LANGUAGES, type ScanStatus, type SubtitleMode } from '@nuvora/core';
import { SubtitleText } from '../player/SubtitleText';
import { deviceLanguage } from '../server';
import { useSignedIn } from '../state';
import { saveSubtitleStyle, subtitleStyle, SUBTITLE_BACKGROUNDS, SUBTITLE_SIZES, type SubtitleStyle } from '../storage';
import { colors, type } from '../theme';
import { Button, FocusGuide, Gradient, Label, ListRow, OptionChips, Panel, SidePanel } from '../ui';
import { Shell } from './Shell';

// eslint-disable-next-line @typescript-eslint/no-var-requires
const APP_VERSION: string = require('../../package.json').version;

const MODES = [
  ['auto', 'Auto'],
  ['always', 'Always'],
  ['off', 'Off'],
] as const;
const MODE_HINTS: Record<SubtitleMode, string> = {
  auto: "On when the audio isn't in your language, or when the file marks a track to show.",
  always: "On whenever there's a track in your language, or one the file marks to show.",
  off: 'Off until you turn them on in the player.',
};

export function Settings() {
  const { client, session, device, signOut, updateUser } = useSignedIn();
  const user = session.user;
  const [look, setLook] = useState<SubtitleStyle>(subtitleStyle);
  const [picking, setPicking] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const tvLanguage = languageName(deviceLanguage());

  const saveSubtitles = (mode: SubtitleMode, language: string | null) => {
    setProblem(null);
    client.updateMe({ subtitle_mode: mode, subtitle_language: language }).then(updateUser, (e: unknown) => setProblem(describeError(e)));
  };
  const changeLook = (next: SubtitleStyle) => {
    setLook(next);
    saveSubtitleStyle(next);
  };

  useEffect(() => {
    if (!picking) return;
    const sub = BackHandler.addEventListener('hardwareBackPress', () => {
      setPicking(false);
      return true;
    });
    return () => sub.remove();
  }, [picking]);

  return (
    <Shell section="settings">
      <FocusGuide autoFocus style={styles.page}>
        <ScrollView contentContainerStyle={styles.list}>
          <Text style={styles.heading}>Settings</Text>
          <Panel>
            <Label text="SIGNED IN" />
            <Text style={styles.name}>{user.display_name}</Text>
            <Text style={styles.secondary}>
              {session.server.name} · {session.server.url.replace(/^https?:\/\//, '')}
            </Text>
            <View style={styles.buttons}>
              <Button label="Sign out" icon="signOut" preferred onPress={() => signOut()} />
              <Button label="Use another server" icon="server" onPress={() => signOut({ changeServer: true })} />
            </View>
          </Panel>
          <Panel>
            <Label text="SUBTITLES" />
            <Text style={styles.title}>When to show them</Text>
            <View style={styles.gap8} />
            <OptionChips options={MODES} selected={user.subtitle_mode} onSelect={(mode) => saveSubtitles(mode, user.subtitle_language ?? null)} />
            <Text style={[styles.secondary, styles.gapTop6]}>{MODE_HINTS[user.subtitle_mode]}</Text>
            <Text style={[styles.title, styles.gapTop18]}>Language</Text>
            <View style={styles.gap8} />
            <View style={styles.buttons0}>
              <Button
                label={user.subtitle_language ? languageName(user.subtitle_language) : `This TV's language (${tvLanguage})`}
                icon="subtitles"
                onPress={() => setPicking(true)}
              />
            </View>
            {problem && <Text style={styles.problem}>{problem}</Text>}
            <Text style={styles.note}>Saved to your account, so the web app and other TVs use it too. The player remembers what you pick for each show or movie.</Text>
          </Panel>
          <Panel>
            <Label text="HOW SUBTITLES LOOK ON THIS TV" />
            <View style={styles.lookRow}>
              <View style={styles.lookOptions}>
                <Text style={styles.title}>Size</Text>
                <OptionChips options={SUBTITLE_SIZES} selected={look.size} onSelect={(size) => changeLook({ ...look, size })} />
                <Text style={[styles.title, styles.gapTop8]}>Background</Text>
                <OptionChips options={SUBTITLE_BACKGROUNDS} selected={look.background} onSelect={(background) => changeLook({ ...look, background })} />
              </View>
              <View style={styles.preview}>
                <Gradient direction="right" stops={[[0, '#3B4A5C'], [0.55, '#B98A4E'], [1, '#E9D7B5']]} />
                <SubtitleText lines={['Where are we going?', 'Somewhere bright.']} look={look} height={202} />
              </View>
            </View>
          </Panel>
          {user.role === 'admin' && <ScanPanel />}
          <Panel>
            <Label text="THIS TV PLAYS" />
            <Text style={styles.plays}>
              {device.video
                .map((v) => {
                  const name = ({ h264: 'H.264', hevc: 'HEVC', av1: 'AV1', vp9: 'VP9' } as Record<string, string>)[v.codec] ?? v.codec;
                  const res = v.max_height >= 2160 ? '4K' : `${v.max_height}p`;
                  const hdr = v.hdr.length ? ' ' + v.hdr.map((h) => h.toUpperCase().replace('_', ' ')).join('/') : '';
                  return `${name} ${res}${v.max_bit_depth > 8 ? ' 10-bit' : ''}${hdr}`;
                })
                .join('   ')}
            </Text>
            <Text style={styles.secondary}>Audio: {device.audio.map((a) => ({ aac: 'AAC', ac3: 'AC-3', eac3: 'E-AC-3' } as Record<string, string>)[a.codec] ?? a.codec).join(', ')}</Text>
            <Text style={styles.note}>Anything else plays too: the server converts just the part this TV can't decode.</Text>
          </Panel>
          <Text style={styles.version}>Nuvora for Vega {APP_VERSION}</Text>
        </ScrollView>
      </FocusGuide>
      {picking && (
        <SidePanel title="Subtitle language">
          <ListRow
            label={`This TV's language (${tvLanguage})`}
            selected={!user.subtitle_language}
            preferred={!user.subtitle_language}
            onPress={() => {
              setPicking(false);
              saveSubtitles(user.subtitle_mode, null);
            }}
          />
          {LANGUAGES.map((l) => (
            <ListRow
              key={l.code}
              label={l.name}
              selected={l.code === user.subtitle_language}
              preferred={l.code === user.subtitle_language}
              onPress={() => {
                setPicking(false);
                saveSubtitles(user.subtitle_mode, l.code);
              }}
            />
          ))}
        </SidePanel>
      )}
    </Shell>
  );
}

/** For admins: scan every library now, and how the last scan went. */
function ScanPanel() {
  const { client } = useSignedIn();
  const [status, setStatus] = useState<ScanStatus | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const running = status?.state === 'running';
  useEffect(() => {
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = () => {
      client.scanStatus().then(
        (s) => {
          if (!alive) return;
          setStatus(s);
          timer = setTimeout(poll, s.state === 'running' ? 1_000 : 5_000);
        },
        () => alive && (timer = setTimeout(poll, 5_000)),
      );
    };
    poll();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [client]);
  const line = !status
    ? '…'
    : status.state === 'running'
      ? ({ discovering: 'Scanning: looking for files…', reading: `Scanning: reading ${status.done} of ${status.total} files`, artwork: 'Scanning: fetching artwork…' } as Record<string, string>)[status.phase] ??
        'Scanning: tidying up…'
      : status.last
        ? lastScan(status.last)
        : 'Not scanned yet.';
  return (
    <Panel>
      <Label text="LIBRARIES" />
      <Text style={styles.title}>{line}</Text>
      <Text style={styles.secondary}>New files show up after a scan. Folders and libraries are set up in a browser, in Settings › Libraries.</Text>
      <View style={styles.buttons}>
        <Button
          label={running ? 'Scanning…' : 'Scan libraries now'}
          icon="restart"
          disabled={running}
          onPress={() => {
            setProblem(null);
            client.startScan().then(setStatus, (e: unknown) => setProblem(describeError(e)));
          }}
        />
      </View>
      {problem && <Text style={styles.problem}>{problem}</Text>}
    </Panel>
  );
}

function lastScan(last: NonNullable<ScanStatus['last']>): string {
  const ago = Math.floor((Date.now() - last.finished_at) / 60_000);
  const when = ago < 1 ? 'just now' : ago < 60 ? `${ago} min ago` : ago < 60 * 24 ? `${Math.floor(ago / 60)} h ago` : `${Math.floor(ago / 1440)} days ago`;
  const changes = [
    last.added > 0 ? `${last.added} added` : null,
    last.changed > 0 ? `${last.changed} changed` : null,
    last.removed > 0 ? `${last.removed} removed` : null,
    last.failed > 0 ? `${last.failed} couldn't be read` : null,
  ].filter(Boolean);
  return `Last scan ${when}: ${changes.join(', ') || 'nothing new'}.`;
}

const styles = StyleSheet.create({
  page: { flex: 1 },
  list: { paddingHorizontal: 56, paddingVertical: 44, gap: 22 },
  heading: { ...type.headlineMedium, color: colors.text },
  name: { ...type.titleLarge, color: colors.text },
  title: { ...type.titleMedium, color: colors.text },
  secondary: { ...type.bodyMedium, color: colors.secondary },
  note: { ...type.bodySmall, color: colors.muted, marginTop: 10 },
  problem: { ...type.bodyMedium, color: colors.danger, marginTop: 6 },
  buttons: { flexDirection: 'row', gap: 14, marginTop: 18 },
  buttons0: { flexDirection: 'row' },
  gap8: { height: 8 },
  gapTop6: { marginTop: 6 },
  gapTop8: { marginTop: 8 },
  gapTop18: { marginTop: 18 },
  lookRow: { flexDirection: 'row', alignItems: 'center' },
  lookOptions: { flex: 1, gap: 8 },
  preview: { width: 360, height: 202, borderRadius: 10, overflow: 'hidden', marginLeft: 24 },
  plays: { ...type.bodyLarge, color: colors.text },
  version: { ...type.bodySmall, color: colors.muted },
});
