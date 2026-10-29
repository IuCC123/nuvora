import React from 'react';
import Svg, { Path } from '@amazon-devices/react-native-svg';
import { ICON_PATHS, type IconName } from './icon-paths';

export type { IconName };

/** The web and Fire TV apps' icon set, Phosphor, in its bold weight for the TV. */
export function Icon({ name, size = 22, color }: { name: IconName; size?: number; color: string }) {
  return (
    // Keyed by name: a changed icon is drawn fresh, not path by path over the old one.
    <Svg key={name} width={size} height={size} viewBox="0 0 256 256">
      {ICON_PATHS[name].map((d, i) => (
        <Path key={i} d={d} fill={color} />
      ))}
    </Svg>
  );
}
