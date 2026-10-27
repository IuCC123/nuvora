import React from 'react';
import Svg, { Path } from '@amazon-devices/react-native-svg';

/** The web and Fire TV apps' icon set: a 24-unit grid, round strokes, from the same SVG paths. */
const stroke = (...d: string[]) => ({ d, filled: false });
const fill = (...d: string[]) => ({ d, filled: true });

const ICONS = {
  home: stroke('M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8', 'M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z'),
  search: stroke('M18 11a7 7 0 1 1-14 0a7 7 0 1 1 14 0z', 'm20 20-3.5-3.5'),
  film: stroke('M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z', 'M7 3v18M3 7.5h4M3 12h18M3 16.5h4M17 3v18M17 7.5h4M17 16.5h4'),
  tv: stroke('M4 4h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z', 'M8 21h8M12 18v3'),
  settings: stroke(
    'M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z',
    'M15 12a3 3 0 1 1-6 0a3 3 0 1 1 6 0z',
  ),
  play: fill('M7 4.5v15a1 1 0 0 0 1.52.85l12.2-7.5a1 1 0 0 0 0-1.7L8.52 3.65A1 1 0 0 0 7 4.5z'),
  pause: fill(
    'M6.7 4h2.1A1.2 1.2 0 0 1 10 5.2v13.6A1.2 1.2 0 0 1 8.8 20H6.7a1.2 1.2 0 0 1-1.2-1.2V5.2A1.2 1.2 0 0 1 6.7 4z',
    'M15.2 4h2.1a1.2 1.2 0 0 1 1.2 1.2v13.6a1.2 1.2 0 0 1-1.2 1.2h-2.1a1.2 1.2 0 0 1-1.2-1.2V5.2A1.2 1.2 0 0 1 15.2 4z',
  ),
  check: stroke('M20 6 9 17l-5-5'),
  skipNext: fill(
    'M4 5.2v13.6a1 1 0 0 0 1.53.85l10.3-6.8a1 1 0 0 0 0-1.7L5.53 4.35A1 1 0 0 0 4 5.2z',
    'M18.6 4h.4a1.1 1.1 0 0 1 1.1 1.1v13.8a1.1 1.1 0 0 1-1.1 1.1h-.4a1.1 1.1 0 0 1-1.1-1.1V5.1A1.1 1.1 0 0 1 18.6 4z',
  ),
  restart: stroke('M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8', 'M3 3v5h5'),
  subtitles: stroke('M5.5 5h13A2.5 2.5 0 0 1 21 7.5v9a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 16.5v-9A2.5 2.5 0 0 1 5.5 5z', 'M6.75 13h3.5M13.25 13h4M6.75 16.2h6.5M16.25 16.2h1'),
  audio: stroke('M11 5 6 9H2v6h4l5 4V5z', 'M15.54 8.46a5 5 0 0 1 0 7.07M19.07 4.93a10 10 0 0 1 0 14.14'),
  server: stroke(
    'M4 2h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z',
    'M4 14h16a2 2 0 0 1 2 2v4a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-4a2 2 0 0 1 2-2z',
    'M6 6h.01M6 18h.01',
  ),
  signOut: stroke('M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9'),
  user: stroke('M17 8a5 5 0 1 1-10 0a5 5 0 1 1 10 0z', 'M20 21a8 8 0 0 0-16 0'),
  back: stroke('m12 19-7-7 7-7M19 12H5'),
  users: stroke('M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2', 'M13 7a4 4 0 1 1-8 0a4 4 0 1 1 8 0z', 'M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75'),
  arrowRight: stroke('M5 12h14M12 5l7 7-7 7'),
  close: stroke('M18 6 6 18M6 6l12 12'),
  layers: stroke('m12 2 10 5-10 5L2 7l10-5z', 'm2 17 10 5 10-5', 'm2 12 10 5 10-5'),
  rewind: stroke('M11 19 2 12l9-7v14z', 'M22 19l-9-7 9-7v14z'),
  forward: stroke('M13 19l9-7-9-7v14z', 'M2 19l9-7-9-7v14z'),
};

export type IconName = keyof typeof ICONS;

export function Icon({ name, size = 22, color }: { name: IconName; size?: number; color: string }) {
  const icon = ICONS[name];
  return (
    // Keyed by name: a changed icon is drawn fresh, not path by path over the old one.
    <Svg key={name} width={size} height={size} viewBox="0 0 24 24">
      {icon.d.map((d, i) =>
        icon.filled ? (
          <Path key={i} d={d} fill={color} />
        ) : (
          <Path key={i} d={d} fill="none" stroke={color} strokeWidth={1.9} strokeLinecap="round" strokeLinejoin="round" />
        ),
      )}
    </Svg>
  );
}
