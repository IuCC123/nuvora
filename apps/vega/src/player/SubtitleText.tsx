import React from 'react';
import { StyleSheet, Text, View } from 'react-native';
import type { SubtitleStyle } from '../storage';

/** Text height as a fraction of the picture's, as on the Fire TV app. */
const SIZE: Record<SubtitleStyle['size'], number> = { small: 0.045, medium: 0.056, large: 0.07 };

/**
 * Subtitle lines over a picture `height` tall, in the look chosen for this
 * TV: plain white, outlined with a shadow, or on a dark box. `lift` keeps
 * them above the player's controls while those show.
 */
export function SubtitleText({ lines, look, height, lift = 0 }: { lines: readonly string[]; look: SubtitleStyle; height: number; lift?: number }) {
  if (!lines.length) return null;
  const fontSize = Math.round(height * SIZE[look.size]);
  return (
    <View style={[styles.area, { bottom: Math.max(height * 0.08, lift) }]} pointerEvents="none">
      {lines.map((line, i) => (
        <Text
          key={i}
          style={[
            styles.line,
            { fontSize, lineHeight: Math.round(fontSize * 1.25) },
            look.background === 'box' && styles.box,
            look.background === 'shadow' && styles.shadow,
          ]}>
          {line}
        </Text>
      ))}
    </View>
  );
}

const styles = StyleSheet.create({
  area: { position: 'absolute', left: '8%', right: '8%', alignItems: 'center' },
  line: { color: '#fff', textAlign: 'center', fontWeight: '500' },
  box: { backgroundColor: 'rgba(0,0,0,0.8)', paddingHorizontal: 8 },
  shadow: { textShadowColor: '#000', textShadowOffset: { width: 0, height: 0 }, textShadowRadius: 4 },
});
