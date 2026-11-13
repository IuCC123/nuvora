import type { CSSProperties } from "react";

/**
 * A person's avatar colors: a gradient whose hue comes from their username,
 * so each household member keeps one recognisable color on every device.
 */
export function avatarStyle(username: string): CSSProperties {
  let hash = 0;
  for (const ch of username) hash = (hash * 31 + ch.codePointAt(0)!) >>> 0;
  const hue = hash % 360;
  return {
    background: `linear-gradient(135deg, oklch(0.74 0.13 ${hue}), oklch(0.58 0.17 ${(hue + 50) % 360}))`,
  };
}
