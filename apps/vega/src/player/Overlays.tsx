import React, { useState } from 'react';
import { Image, StyleSheet, Text, View } from 'react-native';
import { formatClock, languageName, previewFrame, subtitleLabel, type AudioTrack, type PreviewInfo, type SubtitleTrack } from '@nuvora/core';
import { Icon, type IconName } from '../icons';
import { useSignedIn } from '../state';
import { SUBTITLE_BACKGROUNDS, SUBTITLE_SIZES, type SubtitleStyle } from '../storage';
import { colors, radius, type } from '../theme';
import { Button, FocusGuide, Focusable, Gradient, ListRow, OptionChips, SidePanel } from '../ui';

const INSET = 56;

interface ControlsProps {
  title: string;
  subtitle: string | null;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  together: string[];
  hasSubtitles: boolean;
  hasAudioChoice: boolean;
  onToggle(): void;
  onBack(): void;
  onForward(): void;
  onSubtitles(): void;
  onAudio(): void;
  onNext: (() => void) | null;
  /** Any press, so the controls stay a while longer. */
  onAnyKey(): void;
}

/** The title on top, the timeline and buttons at the bottom, over soft shade. */
export function Controls(p: ControlsProps) {
  const fraction = p.durationMs > 0 ? Math.min(1, Math.max(0, p.positionMs / p.durationMs)) : 0;
  const press = (action: () => void) => () => {
    p.onAnyKey();
    action();
  };
  return (
    <View style={StyleSheet.absoluteFill}>
      <View style={styles.topShade}>
        <Gradient direction="down" stops={[[0, '#000', 0.75], [1, '#000', 0]]} />
      </View>
      <View style={styles.bottomShade}>
        <Gradient direction="down" stops={[[0, '#000', 0], [1, '#000', 0.85]]} />
      </View>
      {p.together.length > 0 && (
        <View style={styles.together}>
          <Icon name="users" size={18} color={colors.accent} />
          <Text style={styles.togetherText}>
            With {p.together[0]}
            {p.together.length > 1 ? ` +${p.together.length - 1}` : ''}
          </Text>
        </View>
      )}
      <View style={styles.titles}>
        <Text style={styles.title} numberOfLines={1}>
          {p.title}
        </Text>
        {p.subtitle ? (
          <Text style={styles.subtitle} numberOfLines={1}>
            {p.subtitle}
          </Text>
        ) : null}
      </View>
      <View style={styles.bottom}>
        <Timeline positionMs={p.positionMs} durationMs={p.durationMs} fraction={fraction} />
        <FocusGuide autoFocus trapFocusUp trapFocusDown style={styles.buttons}>
          <RoundButton icon="rewind" onPress={press(p.onBack)} />
          <RoundButton icon={p.playing ? 'pause' : 'play'} big preferred onPress={press(p.onToggle)} />
          <RoundButton icon="forward" onPress={press(p.onForward)} />
          <View style={styles.spacer} />
          {p.hasSubtitles && <Button label="Subtitles" icon="subtitles" onPress={press(p.onSubtitles)} onFocus={p.onAnyKey} />}
          {p.hasAudioChoice && <Button label="Audio" icon="audio" onPress={press(p.onAudio)} onFocus={p.onAnyKey} />}
          {p.onNext && <Button label="Next episode" icon="skipNext" onPress={press(p.onNext)} onFocus={p.onAnyKey} />}
        </FocusGuide>
      </View>
    </View>
  );
}

function Timeline({ positionMs, durationMs, fraction }: { positionMs: number; durationMs: number; fraction: number }) {
  return (
    <View style={styles.timeline}>
      <Text style={styles.clock}>{formatClock(positionMs)}</Text>
      <View style={styles.track}>
        <View style={[styles.played, { width: `${fraction * 100}%` }]} />
      </View>
      <Text style={styles.remaining}>−{formatClock(Math.max(0, durationMs - positionMs))}</Text>
    </View>
  );
}

function RoundButton({ icon, onPress, big, preferred }: { icon: IconName; onPress: () => void; big?: boolean; preferred?: boolean }) {
  const size = big ? 72 : 56;
  return (
    <Focusable onPress={onPress} preferred={preferred} scale={1.1} style={(focused) => [styles.round, { width: size, height: size, borderRadius: size / 2 }, focused && { backgroundColor: colors.text }]}>
      {(focused) => <Icon name={icon} size={big ? 32 : 24} color={focused ? colors.canvas : colors.text} />}
    </Focusable>
  );
}

/** How far one press moves the scrub: further the longer the button is held. */
export function scrubStep(presses: number): number {
  return presses < 8 ? 10_000 : presses < 24 ? 30_000 : 60_000;
}

const FRAME_WIDTH = 288;

/** While scrubbing: where the jump will land, with the frame there once previews exist. */
export function ScrubBar({ targetMs, durationMs, previews }: { targetMs: number; durationMs: number; previews: PreviewInfo | null }) {
  const { client, session } = useSignedIn();
  const fraction = durationMs > 0 ? Math.min(1, Math.max(0, targetMs / durationMs)) : 0;
  const frame = previews ? previewFrame(previews, targetMs / 1000) : null;
  const cardWidth = frame ? FRAME_WIDTH : 96;
  const [width, setWidth] = useState(0);
  const x = Math.min(Math.max(0, width * fraction - cardWidth / 2), Math.max(0, width - cardWidth));
  const inner = FRAME_WIDTH - 10;
  const scale = frame ? inner / frame.width : 1;
  return (
    <View style={StyleSheet.absoluteFill} pointerEvents="none">
      <View style={styles.scrubShade}>
        <Gradient direction="down" stops={[[0, '#000', 0], [1, '#000', 0.85]]} />
      </View>
      <View style={styles.bottom}>
        <View onLayout={(e) => setWidth(e.nativeEvent.layout.width)}>
          <View style={[styles.scrubCard, { width: cardWidth, marginLeft: x }]}>
            {frame && (
              <View style={[styles.frame, { width: inner, height: frame.height * scale }]}>
                <Image
                  source={{ uri: client.resolve(frame.sheet), headers: { Authorization: `Bearer ${session.token}` } }}
                  style={{ position: 'absolute', left: -frame.x * scale, top: -frame.y * scale, width: frame.sheetWidth * scale, height: frame.sheetHeight * scale }}
                />
              </View>
            )}
            <Text style={styles.clock}>{formatClock(targetMs)}</Text>
          </View>
        </View>
        <View style={[styles.track, styles.scrubTrack]}>
          <View style={[styles.played, { width: `${fraction * 100}%` }]} />
        </View>
        <Text style={[styles.remaining, styles.scrubRemaining]}>−{formatClock(Math.max(0, durationMs - targetMs))}</Text>
      </View>
    </View>
  );
}

/** Near the end of an episode: the next one when the count runs out, or now. */
export function UpNext({ label, title, seconds, onPlay, onCancel }: { label: string; title: string; seconds: number; onPlay(): void; onCancel(): void }) {
  return (
    <View style={styles.upNext}>
      <Text style={styles.kicker}>UP NEXT IN {seconds}</Text>
      <Text style={styles.upNextLabel}>{label}</Text>
      <Text style={styles.upNextTitle}>{title}</Text>
      <FocusGuide autoFocus style={styles.upNextButtons}>
        <Button label="Play now" icon="play" primary preferred onPress={onPlay} />
        <Button label="Back" onPress={onCancel} />
      </FocusGuide>
    </View>
  );
}

/** Before a shared start: everyone's player holds, counting down together. */
export function Countdown({ seconds }: { seconds: number }) {
  return (
    <View style={styles.countdown} pointerEvents="none">
      <Text style={styles.kicker}>STARTING TOGETHER</Text>
      <Text style={styles.countdownNumber}>{seconds}</Text>
    </View>
  );
}

/** What someone else in the party just did. */
export function Note({ text }: { text: string }) {
  return (
    <View style={styles.noteWrap} pointerEvents="none">
      <View style={styles.note}>
        <Icon name="users" size={18} color={colors.accent} />
        <Text style={styles.noteText}>{text}</Text>
      </View>
    </View>
  );
}

/** Off and every subtitle track, then how they look. A pick applies at once and is remembered for the show or movie. */
export function SubtitleMenu({ tracks, selected, look, onPick, onLook }: { tracks: readonly SubtitleTrack[]; selected: SubtitleTrack | null; look: SubtitleStyle; onPick(track: SubtitleTrack | null): void; onLook(look: SubtitleStyle): void }) {
  return (
    <SidePanel title="Subtitles">
      <ListRow label="Off" selected={!selected} preferred={!selected} onPress={() => onPick(null)} />
      {tracks.map((track) => {
        const label = subtitleLabel(track);
        return (
          <ListRow
            key={track.index}
            label={[label.name, label.title, ...label.tags].filter(Boolean).join(' · ')}
            selected={selected?.index === track.index}
            preferred={selected?.index === track.index}
            onPress={() => onPick(track)}
          />
        );
      })}
      {tracks.length === 0 && <Text style={styles.empty}>This video has no subtitles.</Text>}
      <View style={styles.lookOptions}>
        <Text style={styles.lookLabel}>Size</Text>
        <OptionChips options={SUBTITLE_SIZES} selected={look.size} onSelect={(size) => onLook({ ...look, size })} />
        <Text style={[styles.lookLabel, styles.lookGap]}>Background</Text>
        <OptionChips options={SUBTITLE_BACKGROUNDS} selected={look.background} onSelect={(background) => onLook({ ...look, background })} />
      </View>
    </SidePanel>
  );
}

/** `"Japanese · Stereo"`, `"English · Commentary · 5.1"`. */
function audioLabel(track: AudioTrack): string {
  const name = languageName(track.language);
  const title = track.title?.trim();
  const channels = ({ 1: 'Mono', 2: 'Stereo', 6: '5.1', 8: '7.1' } as Record<number, string>)[track.channels];
  return [name, title && title.toLowerCase() !== name.toLowerCase() ? title : null, channels].filter(Boolean).join(' · ');
}

/** The file's audio tracks; picking one restarts the stream with it, where it was. */
export function AudioMenu({ tracks, selected, onPick }: { tracks: readonly AudioTrack[]; selected: number | null; onPick(track: AudioTrack): void }) {
  return (
    <SidePanel title="Audio">
      {tracks.map((track) => (
        <ListRow key={track.index} label={audioLabel(track)} selected={track.index === selected} preferred={track.index === selected} onPress={() => onPick(track)} />
      ))}
    </SidePanel>
  );
}

const styles = StyleSheet.create({
  topShade: { position: 'absolute', left: 0, right: 0, top: 0, height: 180 },
  bottomShade: { position: 'absolute', left: 0, right: 0, bottom: 0, height: 260 },
  scrubShade: { position: 'absolute', left: 0, right: 0, bottom: 0, height: 320 },
  together: { position: 'absolute', top: 44, right: INSET, flexDirection: 'row', alignItems: 'center', gap: 8, paddingHorizontal: 16, paddingVertical: 8, borderRadius: radius.pill, backgroundColor: 'rgba(255,255,255,0.12)' },
  togetherText: { ...type.labelLarge, color: colors.text },
  titles: { position: 'absolute', left: INSET, top: 40, right: 280 },
  title: { ...type.headlineMedium, color: colors.text },
  subtitle: { ...type.titleMedium, color: colors.secondary },
  bottom: { position: 'absolute', left: INSET, right: INSET, bottom: 40 },
  timeline: { flexDirection: 'row', alignItems: 'center' },
  clock: { ...type.titleSmall, color: colors.text },
  track: { flex: 1, height: 6, borderRadius: 3, marginHorizontal: 16, backgroundColor: 'rgba(255,255,255,0.25)', overflow: 'hidden' },
  played: { height: '100%', backgroundColor: colors.accent },
  remaining: { ...type.titleSmall, color: colors.secondary },
  buttons: { flexDirection: 'row', alignItems: 'center', gap: 14, marginTop: 22 },
  spacer: { flex: 1 },
  round: { alignItems: 'center', justifyContent: 'center', backgroundColor: 'rgba(255,255,255,0.12)' },
  scrubCard: { alignItems: 'center', borderRadius: radius.lg, backgroundColor: colors.surface, padding: 5 },
  frame: { borderRadius: radius.sm, overflow: 'hidden', marginBottom: 4 },
  scrubTrack: { flex: 0, marginHorizontal: 0, marginTop: 12 },
  scrubRemaining: { textAlign: 'right', marginTop: 10 },
  upNext: { ...StyleSheet.absoluteFillObject, backgroundColor: 'rgba(15,16,18,0.85)', alignItems: 'center', justifyContent: 'center' },
  kicker: { ...type.labelSmall, color: colors.accent },
  upNextLabel: { ...type.titleMedium, color: colors.secondary, marginTop: 10 },
  upNextTitle: { ...type.headlineMedium, color: colors.text },
  upNextButtons: { flexDirection: 'row', gap: 14, marginTop: 26 },
  countdown: { ...StyleSheet.absoluteFillObject, backgroundColor: 'rgba(0,0,0,0.55)', alignItems: 'center', justifyContent: 'center' },
  countdownNumber: { fontSize: 96, lineHeight: 100, fontWeight: '700', color: colors.text, marginTop: 8 },
  noteWrap: { position: 'absolute', top: 36, left: 0, right: 0, alignItems: 'center' },
  note: { flexDirection: 'row', alignItems: 'center', gap: 10, paddingHorizontal: 22, paddingVertical: 10, borderRadius: radius.pill, backgroundColor: 'rgba(26,28,31,0.92)' },
  noteText: { ...type.titleSmall, color: colors.text },
  empty: { ...type.bodyMedium, color: colors.muted, padding: 16 },
  lookOptions: { marginTop: 24, gap: 10 },
  lookLabel: { ...type.labelLarge, color: colors.secondary },
  lookGap: { marginTop: 4 },
});
