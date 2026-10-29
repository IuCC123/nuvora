import * as Ph from "@phosphor-icons/react";

/**
 * The app's icons, from Phosphor (phosphoricons.com). Strokes are optically
 * sized: bold under 20px, regular from 20px up. Solid controls (play, pause)
 * and "on" states use the fill weight. Decorative unless labelled by their control.
 */
export type IconProps = Omit<Ph.IconProps, "ref">;

export function opticalWeight(size: IconProps["size"]): Ph.IconWeight {
  return Number(size) < 20 ? "bold" : "regular";
}

function icon(Glyph: Ph.Icon, defaults: { size?: number; weight?: Ph.IconWeight } = {}) {
  return function AppIcon({ size = defaults.size ?? 20, weight = defaults.weight ?? opticalWeight(size), ...rest }: IconProps) {
    return <Glyph size={size} weight={weight} aria-hidden focusable={false} {...rest} />;
  };
}

/** The brand mark, not an icon: the wordmark's rounded play triangle. */
export function MarkIcon() {
  return (
    <svg width={16} height={18} viewBox="0 0 16 18" aria-hidden focusable={false}>
      <path d="M1.5 1.6v14.8a1 1 0 0 0 1.5.86l12-7.4a1 1 0 0 0 0-1.72L3 .74A1 1 0 0 0 1.5 1.6z" fill="currentColor" />
    </svg>
  );
}

export const PlayIcon = icon(Ph.PlayIcon, { size: 18, weight: "fill" });
export const PauseIcon = icon(Ph.PauseIcon, { size: 18, weight: "fill" });
export const SkipNextIcon = icon(Ph.SkipForwardIcon, { size: 22, weight: "fill" });
export const StarIcon = icon(Ph.StarIcon, { weight: "fill" });
export const ResumeIcon = icon(Ph.PlayCircleIcon);
export const RestartIcon = icon(Ph.ArrowCounterClockwiseIcon, { size: 18 });
export const HouseIcon = icon(Ph.HouseIcon);
export const FilmIcon = icon(Ph.FilmSlateIcon);
export const TvIcon = icon(Ph.TelevisionIcon);
export const SearchIcon = icon(Ph.MagnifyingGlassIcon);
export const SettingsIcon = icon(Ph.GearSixIcon);
export const SlidersIcon = icon(Ph.SlidersHorizontalIcon);
export const ActivityIcon = icon(Ph.PulseIcon);
export const DownloadIcon = icon(Ph.DownloadSimpleIcon);
export const ServerIcon = icon(Ph.HardDrivesIcon);
export const DriveIcon = icon(Ph.HardDriveIcon);
export const FolderIcon = icon(Ph.FolderSimpleIcon);
export const ParentFolderIcon = icon(Ph.ArrowBendLeftUpIcon, { size: 18 });
export const UserIcon = icon(Ph.UserIcon);
export const UsersIcon = icon(Ph.UsersIcon);
export const SignOutIcon = icon(Ph.SignOutIcon, { size: 18 });
export const ClockIcon = icon(Ph.ClockIcon);
export const EyeIcon = icon(Ph.EyeIcon);
export const EyeOffIcon = icon(Ph.EyeSlashIcon);
export const PencilIcon = icon(Ph.PencilSimpleIcon);
export const TrashIcon = icon(Ph.TrashIcon);
export const GripIcon = icon(Ph.DotsSixVerticalIcon);
export const RefreshIcon = icon(Ph.ArrowsClockwiseIcon, { size: 18 });
export const SpinIcon = icon(Ph.CircleNotchIcon, { size: 16 });
export const AlertIcon = icon(Ph.WarningCircleIcon, { size: 18 });
export const CheckIcon = icon(Ph.CheckIcon, { size: 16, weight: "bold" });
export const CloseIcon = icon(Ph.XIcon, { size: 16 });
export const PlusIcon = icon(Ph.PlusIcon, { size: 18 });
export const ChevronDownIcon = icon(Ph.CaretDownIcon, { size: 16 });
export const ChevronLeftIcon = icon(Ph.CaretLeftIcon, { size: 18 });
export const ChevronRightIcon = icon(Ph.CaretRightIcon, { size: 18 });
export const ArrowLeftIcon = icon(Ph.ArrowLeftIcon, { size: 22 });
export const ArrowUpIcon = icon(Ph.ArrowUpIcon);
export const ArrowDownIcon = icon(Ph.ArrowDownIcon);
export const PipIcon = icon(Ph.PictureInPictureIcon, { size: 22 });

const SortAscending = icon(Ph.SortAscendingIcon, { size: 18 });
const SortDescending = icon(Ph.SortDescendingIcon, { size: 18 });
export function SortDirectionIcon({ descending, ...p }: IconProps & { descending: boolean }) {
  return descending ? <SortDescending {...p} /> : <SortAscending {...p} />;
}

const SpeakerX = icon(Ph.SpeakerXIcon, { size: 22 });
const SpeakerLow = icon(Ph.SpeakerLowIcon, { size: 22 });
const SpeakerHigh = icon(Ph.SpeakerHighIcon, { size: 22 });
export function VolumeIcon({ level, ...p }: IconProps & { level: "muted" | "low" | "high" }) {
  const Speaker = level === "muted" ? SpeakerX : level === "low" ? SpeakerLow : SpeakerHigh;
  return <Speaker {...p} />;
}

const CornersOut = icon(Ph.CornersOutIcon, { size: 22 });
const CornersIn = icon(Ph.CornersInIcon, { size: 22 });
export function FullscreenIcon({ exit, ...p }: IconProps & { exit: boolean }) {
  return exit ? <CornersIn {...p} /> : <CornersOut {...p} />;
}

const Captions = icon(Ph.ClosedCaptioningIcon, { size: 22 });
export const CaptionsIcon = icon(Ph.ClosedCaptioningIcon);
/** Closed captions: solid while subtitles are on. */
export function SubtitlesIcon({ on, ...p }: IconProps & { on: boolean }) {
  return <Captions weight={on ? "fill" : undefined} {...p} />;
}

/** A circular arrow with "10" inside its open centre: back when `forward` is false. */
const Back10 = icon(Ph.ArrowCounterClockwiseIcon, { size: 28 });
const Forward10 = icon(Ph.ArrowClockwiseIcon, { size: 28 });
function Skip10({ forward, ...p }: IconProps & { forward: boolean }) {
  const Arrow = forward ? Forward10 : Back10;
  return (
    <Arrow {...p}>
      <text x="128" y="155" textAnchor="middle" fontSize="76" fontWeight="700" style={{ fontVariantNumeric: "tabular-nums" }}>
        10
      </text>
    </Arrow>
  );
}

export const Back10Icon = (p: IconProps) => <Skip10 forward={false} {...p} />;
export const Forward10Icon = (p: IconProps) => <Skip10 forward {...p} />;
