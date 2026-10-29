import type { Icon, IconProps as PhosphorProps, IconWeight } from "@phosphor-icons/react";
import * as Ph from "@phosphor-icons/react/ssr";

/* The web app's own icons (apps/web/src/components/Icon.tsx), so the illustrations show the real controls. */

type IconProps = Omit<PhosphorProps, "ref">;

function icon(Glyph: Icon, defaults: { size?: number; weight?: IconWeight } = {}) {
  return function AppIcon({ size = defaults.size ?? 20, weight = defaults.weight ?? (Number(size) < 20 ? "bold" : "regular"), ...rest }: IconProps) {
    return <Glyph size={size} weight={weight} aria-hidden focusable={false} {...rest} />;
  };
}

export const PlayIcon = icon(Ph.PlayIcon, { size: 18, weight: "fill" });
export const PauseIcon = icon(Ph.PauseIcon, { size: 18, weight: "fill" });
export const HouseIcon = icon(Ph.HouseIcon);
export const FilmIcon = icon(Ph.FilmSlateIcon);
export const TvIcon = icon(Ph.TelevisionIcon);
export const SearchIcon = icon(Ph.MagnifyingGlassIcon);
export const SlidersIcon = icon(Ph.SlidersHorizontalIcon);
export const UsersIcon = icon(Ph.UsersIcon);
export const CheckIcon = icon(Ph.CheckIcon, { weight: "bold" });
export const ChevronDownIcon = icon(Ph.CaretDownIcon);
export const ChevronLeftIcon = icon(Ph.CaretLeftIcon, { weight: "bold" });
export const ChevronRightIcon = icon(Ph.CaretRightIcon, { weight: "bold" });
export const ArrowLeftIcon = icon(Ph.ArrowLeftIcon, { size: 22 });
export const VolumeIcon = icon(Ph.SpeakerHighIcon, { size: 22 });
export const PipIcon = icon(Ph.PictureInPictureIcon, { size: 22 });
export const FullscreenIcon = icon(Ph.CornersOutIcon, { size: 22 });

const Captions = icon(Ph.ClosedCaptioningIcon, { size: 22 });
export function SubtitlesIcon({ on, ...p }: IconProps & { on: boolean }) {
  return <Captions weight={on ? "fill" : undefined} {...p} />;
}

const Back10 = icon(Ph.ArrowCounterClockwiseIcon, { size: 28 });
const Forward10 = icon(Ph.ArrowClockwiseIcon, { size: 28 });
function Skip10({ forward, ...p }: IconProps & { forward: boolean }) {
  const Arrow = forward ? Forward10 : Back10;
  return (
    <Arrow {...p}>
      <text x="128" y="155" textAnchor="middle" fontSize="76" fontWeight="700">
        10
      </text>
    </Arrow>
  );
}

export const Back10Icon = (p: IconProps) => <Skip10 forward={false} {...p} />;
export const Forward10Icon = (p: IconProps) => <Skip10 forward {...p} />;
