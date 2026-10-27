import React, { type ReactNode } from 'react';
import { FlatList, ScrollView, StyleSheet, Text, View } from 'react-native';
import { formatBytes, initials, languageName, type CastMember, type Image, type Playback, type Version } from '@nuvora/core';
import { colors, radius, type } from '../theme';
import { Artwork, FocusGuide, Focusable, Gradient, ListRow, SidePanel, TitleCard } from '../ui';

/** Detail pages sit further in from the edge than the rows, as on the Fire TV app. */
export const DETAIL_INSET = 56;

/** The server can stream it to this TV. */
export const playable = (p: Playback) => p.kind === 'hls' || p.kind === 'direct_play';

/** Why it can't, in the server's words. */
export const notPlayableReasons = (p: Playback | undefined): string[] =>
  !p ? ["Its files are missing from the server."] : p.kind === 'needs_transcode' ? p.reasons : p.kind === 'unavailable' ? [p.reason] : [];

interface LayoutProps {
  backdrop: Image | null | undefined;
  poster: Image | null | undefined;
  title: string;
  year: number | null | undefined;
  meta: string[];
  tagline?: string | null;
  genres: string[];
  overview?: string | null;
  actions: ReactNode;
  children?: ReactNode;
}

/**
 * A title's page: the backdrop across the screen, darkened where the text
 * sits; the poster and the details in front; seasons, episodes and cast
 * underneath.
 */
export function DetailLayout({ backdrop, poster, title, year, meta, tagline, genres, overview, actions, children }: LayoutProps) {
  return (
    <View style={styles.screen}>
      {backdrop && <Artwork image={backdrop} size="large" style={styles.backdrop} />}
      <GradientOverlays />
      <ScrollView contentContainerStyle={styles.scroll}>
        <View style={styles.top}>
          <View style={styles.poster}>{poster ? <Artwork image={poster} size="large" style={StyleSheet.absoluteFill} /> : <TitleCard title={title} year={year} />}</View>
          <View style={styles.info}>
            <Text style={styles.title} numberOfLines={2}>
              {title}
            </Text>
            {meta.length > 0 && <Text style={styles.meta}>{meta.join('  ·  ')}</Text>}
            {genres.length > 0 && <Text style={styles.genres}>{genres.join(', ')}</Text>}
            {tagline ? <Text style={styles.tagline}>{tagline}</Text> : null}
            {overview ? (
              <Text style={styles.overview} numberOfLines={5}>
                {overview}
              </Text>
            ) : null}
            <FocusGuide autoFocus style={styles.actions}>
              {actions}
            </FocusGuide>
          </View>
        </View>
        {children}
      </ScrollView>
    </View>
  );
}

function GradientOverlays() {
  return (
    <>
      <Gradient direction="right" stops={[[0, colors.canvas, 1], [0.45, colors.canvas, 0.92], [1, colors.canvas, 0]]} />
      <Gradient direction="down" stops={[[0.35, colors.canvas, 0], [0.8, colors.canvas, 1]]} />
    </>
  );
}

/** The cast, as round photos with who they play. */
export function CastRow({ cast }: { cast: readonly CastMember[] }) {
  const seen = new Set<string>();
  const people = cast.filter((m) => !seen.has(m.name + m.character) && seen.add(m.name + m.character)).slice(0, 30);
  if (!people.length) return null;
  return (
    <View>
      <Text style={styles.castTitle}>Cast</Text>
      <FocusGuide autoFocus>
        <FlatList
          horizontal
          data={people}
          keyExtractor={(m, i) => `${m.name}|${m.character ?? ''}|${i}`}
          showsHorizontalScrollIndicator={false}
          contentContainerStyle={styles.castList}
          initialNumToRender={8}
          maxToRenderPerBatch={8}
          windowSize={5}
          renderItem={({ item }) => {
            const face = item.image ?? item.character_image;
            return (
              <View style={styles.member}>
                <Focusable scale={1.06} style={styles.face}>
                  {(focused) => (
                    <>
                      {face ? <Artwork image={face} style={StyleSheet.absoluteFill} /> : <Text style={styles.faceText}>{initials(item.name)}</Text>}
                      {focused && <View style={styles.faceRing} />}
                    </>
                  )}
                </Focusable>
                <Text style={styles.memberName} numberOfLines={1}>
                  {item.name}
                </Text>
                {item.character ? (
                  <Text style={styles.memberRole} numberOfLines={1}>
                    {item.character}
                  </Text>
                ) : null}
              </View>
            );
          }}
        />
      </FocusGuide>
    </View>
  );
}

/** Why nothing plays here, when the server says so. */
export function NotPlayable({ reasons }: { reasons: string[] }) {
  return (
    <View style={styles.notPlayable}>
      <Text style={styles.notPlayableTitle}>This can't play on this TV</Text>
      {reasons.map((r) => (
        <Text key={r} style={styles.notPlayableReason}>
          {r}
        </Text>
      ))}
    </View>
  );
}

/** What's in a file: its audio and subtitle tracks, its name and size. */
export function VersionFacts({ version }: { version: Version }) {
  const media = version.media;
  if (!media) return null;
  const audio = media.audio.map((a) => [a.language ? languageName(a.language) : null, a.title?.trim() || null, a.layout ?? `${a.channels} ch`].filter(Boolean).join(' '));
  const subs = [...new Set(media.subtitles.map((t) => (t.language ? languageName(t.language) : (t.title ?? t.codec))))];
  return (
    <View style={styles.facts}>
      {audio.length > 0 && <Fact label="Audio" value={audio.join('  ·  ')} />}
      {subs.length > 0 && <Fact label="Subtitles" value={subs.slice(0, 8).join(', ') + (subs.length > 8 ? ` +${subs.length - 8}` : '')} />}
      <Fact label="File" value={[version.file_name, version.size_bytes > 0 ? formatBytes(version.size_bytes) : null].filter(Boolean).join('  ·  ')} />
    </View>
  );
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.fact}>
      <Text style={styles.factLabel}>{label}</Text>
      <Text style={styles.factValue} numberOfLines={1}>
        {value}
      </Text>
    </View>
  );
}

/** The versions of a movie or episode, to play a different one. */
export function VersionPicker({ versions, chosen, onPick }: { versions: readonly Version[]; chosen: number; onPick: (version: Version) => void }) {
  return (
    <SidePanel title="Version" width={560}>
      {versions.map((v) => (
        <ListRow
          key={v.file_id}
          label={v.summary || v.file_name}
          detail={[v.summary ? v.file_name : null, v.size_bytes > 0 ? formatBytes(v.size_bytes) : null, v.playback.kind === 'hls' && v.playback.converts_video ? 'converted for this TV' : null, playable(v.playback) ? null : "can't play here"]
            .filter(Boolean)
            .join(' · ')}
          selected={v.file_id === chosen}
          preferred={v.file_id === chosen}
          disabled={!playable(v.playback)}
          onPress={() => onPick(v)}
        />
      ))}
    </SidePanel>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.canvas },
  backdrop: { position: 'absolute', top: 0, right: 0, width: '100%', aspectRatio: 16 / 9 },
  scroll: { paddingTop: 56, paddingBottom: 48, gap: 32 },
  top: { flexDirection: 'row', paddingHorizontal: DETAIL_INSET },
  poster: { width: 220, aspectRatio: 2 / 3, borderRadius: radius.md, overflow: 'hidden', backgroundColor: colors.surface, marginRight: 40 },
  info: { flex: 1 },
  title: { ...type.display, color: colors.text },
  meta: { ...type.titleMedium, color: colors.secondary, marginTop: 10 },
  genres: { ...type.bodyMedium, color: colors.muted, marginTop: 6 },
  tagline: { ...type.titleMedium, color: colors.text, marginTop: 14 },
  overview: { ...type.bodyLarge, color: colors.secondary, marginTop: 12 },
  actions: { flexDirection: 'row', flexWrap: 'wrap', alignItems: 'center', gap: 14, marginTop: 24 },
  castTitle: { ...type.titleLarge, color: colors.text, marginLeft: DETAIL_INSET, marginBottom: 14 },
  castList: { paddingHorizontal: DETAIL_INSET, paddingVertical: 8, gap: 22 },
  member: { width: 120, alignItems: 'center' },
  face: { width: 104, height: 104, borderRadius: 52, overflow: 'hidden', alignItems: 'center', justifyContent: 'center', backgroundColor: colors.raised },
  faceRing: { ...StyleSheet.absoluteFillObject, borderRadius: 52, borderWidth: 3, borderColor: colors.accent },
  faceText: { ...type.titleMedium, color: colors.secondary },
  memberName: { ...type.labelLarge, color: colors.text, marginTop: 8 },
  memberRole: { ...type.bodySmall, color: colors.muted },
  notPlayable: { borderRadius: radius.lg, backgroundColor: colors.surface, padding: 16 },
  notPlayableTitle: { ...type.titleMedium, color: colors.text },
  notPlayableReason: { ...type.bodyMedium, color: colors.secondary },
  facts: { paddingHorizontal: DETAIL_INSET, gap: 6 },
  fact: { flexDirection: 'row' },
  factLabel: { ...type.labelLarge, color: colors.muted, width: 110 },
  factValue: { ...type.bodyMedium, color: colors.secondary, flex: 1 },
});
