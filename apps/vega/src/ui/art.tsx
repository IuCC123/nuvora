import React, {useId, useRef} from 'react';
import {
  Animated,
  Image,
  StyleSheet,
  Text,
  View,
  type StyleProp,
  type ViewStyle,
} from 'react-native';
import Svg, {
  Defs,
  LinearGradient,
  RadialGradient,
  Rect,
  Stop,
} from '@amazon-devices/react-native-svg';
import {
  initials,
  placeholderDataUrl,
  type Image as ArtworkImage,
} from '@nuvora/core';
import {Icon} from '../icons';
import {imageSource} from '../server';
import {useSignedIn} from '../state';
import {colors, radius, type} from '../theme';

/** An artwork: its ThumbHash blur at once, the image fading in over it. */
export function Artwork({
  image,
  style,
  size = 'small',
}: {
  image: ArtworkImage | null | undefined;
  style?: StyleProp<ViewStyle>;
  size?: 'small' | 'large';
}) {
  const {session} = useSignedIn();
  const shown = useRef(new Animated.Value(0)).current;
  return (
    <View style={[styles.artwork, style]}>
      {image && (
        <>
          {image.placeholder ? (
            <Image
              source={{uri: placeholderDataUrl(image)}}
              style={StyleSheet.absoluteFill}
              resizeMode="cover"
            />
          ) : null}
          <Animated.Image
            source={imageSource(session.server.url, session.token, image, size)}
            style={[StyleSheet.absoluteFill, {opacity: shown}]}
            resizeMode="cover"
            onLoad={() =>
              Animated.timing(shown, {
                toValue: 1,
                duration: 220,
                useNativeDriver: true,
              }).start()
            }
          />
        </>
      )}
    </View>
  );
}

/** A poster for something without artwork: its title, lettered. */
export function TitleCard({
  title,
  year,
}: {
  title: string;
  year: number | null | undefined;
}) {
  return (
    <View style={styles.titleCard}>
      <Gradient
        direction="down"
        stops={[
          [0, '#2A2D33'],
          [1, '#17191C'],
        ]}
      />
      <Text style={styles.titleCardText} numberOfLines={4}>
        {title.toUpperCase()}
      </Text>
      {year != null && <Text style={styles.titleCardYear}>{year}</Text>}
    </View>
  );
}

type GradientStop = [offset: number, color: string, opacity?: number];

/** A linear wash across the parent, top to bottom or left to right. */
export function Gradient({
  direction,
  stops,
  style,
}: {
  direction: 'down' | 'right';
  stops: GradientStop[];
  style?: StyleProp<ViewStyle>;
}) {
  const id = 'g' + useId().replace(/[^a-zA-Z0-9]/g, '');
  return (
    // In a view of its own: a percentage size on the Svg would resolve against the parent's padding.
    <View style={[StyleSheet.absoluteFill, style]} pointerEvents="none">
      <Svg width="100%" height="100%">
        <Defs>
          <LinearGradient
            id={id}
            x1="0"
            y1="0"
            x2={direction === 'right' ? '1' : '0'}
            y2={direction === 'down' ? '1' : '0'}>
            {stops.map(([offset, color, opacity = 1], i) => (
              <Stop
                key={i}
                offset={offset}
                stopColor={color}
                stopOpacity={opacity}
              />
            ))}
          </LinearGradient>
        </Defs>
        <Rect x="0" y="0" width="100%" height="100%" fill={`url(#${id})`} />
      </Svg>
    </View>
  );
}

/** A soft round glow from (`cx`, `cy`), in fractions of the parent. */
export function Glow({
  color,
  cx,
  cy,
  r,
}: {
  color: string;
  cx: number;
  cy: number;
  r: number;
}) {
  const id = 'r' + useId().replace(/[^a-zA-Z0-9]/g, '');
  return (
    <View style={StyleSheet.absoluteFill} pointerEvents="none">
      <Svg width="100%" height="100%">
        <Defs>
          <RadialGradient
            id={id}
            cx={cx}
            cy={cy}
            r={r}
            fx={cx}
            fy={cy}
            gradientUnits="objectBoundingBox">
            <Stop offset={0} stopColor={color} stopOpacity={1} />
            <Stop offset={1} stopColor={colors.canvas} stopOpacity={0} />
          </RadialGradient>
        </Defs>
        <Rect x="0" y="0" width="100%" height="100%" fill={`url(#${id})`} />
      </Svg>
    </View>
  );
}

export function ProgressBar({
  fraction,
  style,
}: {
  fraction: number;
  style?: StyleProp<ViewStyle>;
}) {
  return (
    <View style={[styles.progress, style]}>
      <View
        style={[
          styles.progressFill,
          {
            width: `${Math.round(
              Math.min(1, Math.max(0.02, fraction)) * 100,
            )}%`,
          },
        ]}
      />
    </View>
  );
}

export function Badge({
  text,
  style,
}: {
  text: string;
  style?: StyleProp<ViewStyle>;
}) {
  return (
    <View style={[styles.badge, style]}>
      <Text style={styles.badgeText}>{text}</Text>
    </View>
  );
}

/** Watched: an amber disc with a check. */
export function WatchedMark({
  size = 22,
  style,
}: {
  size?: number;
  style?: StyleProp<ViewStyle>;
}) {
  return (
    <View
      style={[
        styles.watched,
        {width: size, height: size, borderRadius: size / 2},
        style,
      ]}>
      <Icon
        name="check"
        size={Math.round(size * 0.64)}
        color={colors.accentText}
      />
    </View>
  );
}

/** A round badge with someone's initials. */
export function Avatar({
  name,
  size = 44,
  dim,
}: {
  name: string;
  size?: number;
  dim?: boolean;
}) {
  return (
    <View
      style={[
        styles.avatar,
        {width: size, height: size, borderRadius: size / 2},
        dim && {opacity: 0.5},
      ]}>
      <Text style={styles.avatarText}>{initials(name)}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  artwork: {backgroundColor: colors.surface, overflow: 'hidden'},
  titleCard: {
    flex: 1,
    justifyContent: 'flex-end',
    padding: 12,
    overflow: 'hidden',
  },
  titleCardText: {...type.titleMedium, fontWeight: '700', color: colors.text},
  titleCardYear: {...type.labelSmall, color: colors.secondary},
  progress: {height: 4, backgroundColor: 'rgba(0,0,0,0.6)', overflow: 'hidden'},
  progressFill: {height: '100%', backgroundColor: colors.accent},
  badge: {
    borderRadius: radius.pill,
    backgroundColor: 'rgba(0,0,0,0.72)',
    paddingHorizontal: 8,
    paddingVertical: 2,
  },
  badgeText: {...type.labelMedium, color: colors.text},
  watched: {
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: colors.accent,
  },
  avatar: {
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: colors.hairline,
  },
  avatarText: {...type.labelLarge, color: colors.text},
});
