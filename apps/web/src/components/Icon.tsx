import type { SVGProps } from "react";

/** The mockup's icon set: 24-unit grid, round 1.75 to 2 strokes. Decorative unless labelled by their control. */
type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function stroke({ size = 20, strokeWidth = 1.75, ...rest }: IconProps) {
  return {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
    focusable: false,
    ...rest,
  };
}

export function PlayIcon({ size = 18, ...rest }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden focusable={false} {...rest}>
      <path d="M7 4.5v15a1 1 0 0 0 1.52.85l12.2-7.5a1 1 0 0 0 0-1.7L8.52 3.65A1 1 0 0 0 7 4.5z" fill="currentColor" />
    </svg>
  );
}

export function MarkIcon() {
  return (
    <svg width={16} height={18} viewBox="0 0 16 18" aria-hidden focusable={false}>
      <path d="M1.5 1.6v14.8a1 1 0 0 0 1.5.86l12-7.4a1 1 0 0 0 0-1.72L3 .74A1 1 0 0 0 1.5 1.6z" fill="currentColor" />
    </svg>
  );
}

export function HouseIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8" />
      <path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </svg>
  );
}

export function FilmIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <rect width="18" height="18" x="3" y="3" rx="2" />
      <path d="M7 3v18M3 7.5h4M3 12h18M3 16.5h4M17 3v18M17 7.5h4M17 16.5h4" />
    </svg>
  );
}

export function TvIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <rect width="20" height="14" x="2" y="4" rx="2" />
      <path d="M8 21h8M12 18v3" />
    </svg>
  );
}

export function ChevronDownIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 16, strokeWidth: 2, ...p })}>
      <path d="m6 9 6 6 6-6" />
    </svg>
  );
}

/** An arrow beside a short and a long bar: the sort direction, upside down for descending. */
export function SortDirectionIcon({ descending, ...p }: IconProps & { descending: boolean }) {
  return (
    <svg {...stroke({ size: 18, strokeWidth: 2, ...p })}>
      <g transform={descending ? "matrix(1 0 0 -1 0 24)" : undefined}>
        <path d="m3 8 4-4 4 4M7 4v16" />
      </g>
      <path d={descending ? "M15 6h6M15 12h4M15 18h2" : "M15 6h2M15 12h4M15 18h6"} />
    </svg>
  );
}

export function CloseIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 16, strokeWidth: 2, ...p })}>
      <path d="M18 6 6 18M6 6l12 12" />
    </svg>
  );
}

export function RefreshIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 18, ...p })}>
      <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
      <path d="M21 3v5h-5" />
      <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
      <path d="M8 16H3v5" />
    </svg>
  );
}

export function SpinIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 16, strokeWidth: 2.2, ...p })}>
      <path d="M21 12a9 9 0 1 1-6.22-8.56" />
    </svg>
  );
}

export function ChevronLeftIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 18, strokeWidth: 2, ...p })}>
      <path d="m15 18-6-6 6-6" />
    </svg>
  );
}

export function ChevronRightIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 18, strokeWidth: 2, ...p })}>
      <path d="m9 18 6-6-6-6" />
    </svg>
  );
}

export function AlertIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 18, strokeWidth: 1.9, ...p })}>
      <circle cx="12" cy="12" r="10" />
      <path d="M12 8v4M12 16h.01" />
    </svg>
  );
}

export function SettingsIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

export function ServerIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <rect width="20" height="8" x="2" y="2" rx="2" />
      <rect width="20" height="8" x="2" y="14" rx="2" />
      <path d="M6 6h.01M6 18h.01" />
    </svg>
  );
}

export function FolderIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
    </svg>
  );
}

export function UsersIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
      <circle cx="9" cy="7" r="4" />
      <path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" />
    </svg>
  );
}

export function UserIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <circle cx="12" cy="8" r="5" />
      <path d="M20 21a8 8 0 0 0-16 0" />
    </svg>
  );
}

export function SignOutIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 18, ...p })}>
      <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9" />
    </svg>
  );
}

export function PlusIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 18, strokeWidth: 2, ...p })}>
      <path d="M5 12h14M12 5v14" />
    </svg>
  );
}

export function ParentFolderIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 18, ...p })}>
      <path d="M14 9 9 4 4 9M20 20h-7a4 4 0 0 1-4-4V4" />
    </svg>
  );
}

export function DriveIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M22 12H2M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11zM6 16h.01M10 16h.01" />
    </svg>
  );
}

export function CheckIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 16, strokeWidth: 2.2, ...p })}>
      <path d="M20 6 9 17l-5-5" />
    </svg>
  );
}

export function PauseIcon({ size = 18, ...rest }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden focusable={false} {...rest}>
      <rect x="5.5" y="4" width="4.5" height="16" rx="1.2" fill="currentColor" />
      <rect x="14" y="4" width="4.5" height="16" rx="1.2" fill="currentColor" />
    </svg>
  );
}

/** Play then a bar: skip to the next episode. */
export function SkipNextIcon({ size = 22, ...rest }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden focusable={false} {...rest}>
      <path d="M4 5.2v13.6a1 1 0 0 0 1.53.85l10.3-6.8a1 1 0 0 0 0-1.7L5.53 4.35A1 1 0 0 0 4 5.2z" fill="currentColor" />
      <rect x="17.5" y="4" width="2.6" height="16" rx="1.1" fill="currentColor" />
    </svg>
  );
}

export function ArrowLeftIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 22, strokeWidth: 2, ...p })}>
      <path d="m12 19-7-7 7-7M19 12H5" />
    </svg>
  );
}

/** A circular arrow with "10" inside: back when `forward` is false. */
function Skip10({ forward, ...p }: IconProps & { forward: boolean }) {
  return (
    <svg {...stroke({ size: 28, strokeWidth: 1.9, ...p })}>
      <g transform={forward ? "matrix(-1 0 0 1 24 0)" : undefined}>
        <path d="M4.2 8.5A8.8 8.8 0 1 1 3.4 13" />
        <path d="M3.6 3.8v4.9h4.9" />
      </g>
      <text x="12.4" y="15.6" textAnchor="middle" fontSize="7.4" fontWeight="700" fill="currentColor" stroke="none" style={{ fontVariantNumeric: "tabular-nums" }}>
        10
      </text>
    </svg>
  );
}

export const Back10Icon = (p: IconProps) => <Skip10 forward={false} {...p} />;
export const Forward10Icon = (p: IconProps) => <Skip10 forward {...p} />;

/** Speaker with 0, 1 or 2 waves, or crossed out when muted. */
export function VolumeIcon({ level, ...p }: IconProps & { level: "muted" | "low" | "high" }) {
  return (
    <svg {...stroke({ size: 22, ...p })}>
      <path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A1.4 1.4 0 0 1 5.4 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.4a1.4 1.4 0 0 1 1 .4l3.4 3.4a.7.7 0 0 0 1.2-.5z" fill="currentColor" />
      {level === "muted" ? (
        <path d="m16 9 6 6M22 9l-6 6" />
      ) : (
        <>
          <path d="M15.5 8.5a5 5 0 0 1 0 7" />
          {level === "high" && <path d="M18.7 5.3a9.5 9.5 0 0 1 0 13.4" />}
        </>
      )}
    </svg>
  );
}

export function PipIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 22, ...p })}>
      <path d="M21 10V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h5" />
      <rect x="13" y="13" width="9" height="7" rx="1.5" fill="currentColor" stroke="none" />
    </svg>
  );
}

export function FullscreenIcon({ exit, ...p }: IconProps & { exit: boolean }) {
  return (
    <svg {...stroke({ size: 22, strokeWidth: 2, ...p })}>
      <path d={exit ? "M8 3v3a2 2 0 0 1-2 2H3M21 8h-3a2 2 0 0 1-2-2V3M3 16h3a2 2 0 0 1 2 2v3M16 21v-3a2 2 0 0 1 2-2h3" : "M8 3H5a2 2 0 0 0-2 2v3M21 8V5a2 2 0 0 0-2-2h-3M3 16v3a2 2 0 0 0 2 2h3M16 21h3a2 2 0 0 0 2-2v-3"} />
    </svg>
  );
}

export function RestartIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 18, strokeWidth: 2, ...p })}>
      <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
    </svg>
  );
}

/** A caption box: two lines of text, solid while subtitles are on. */
export function SubtitlesIcon({ on, ...p }: IconProps & { on: boolean }) {
  return (
    <svg {...stroke({ size: 22, ...p })}>
      {on ? (
        <path
          fillRule="evenodd"
          fill="currentColor"
          stroke="none"
          d="M5 4.5h14a2.5 2.5 0 0 1 2.5 2.5v10a2.5 2.5 0 0 1-2.5 2.5H5A2.5 2.5 0 0 1 2.5 17V7A2.5 2.5 0 0 1 5 4.5zM6.75 12.1a.9.9 0 0 0 0 1.8h3.5a.9.9 0 0 0 0-1.8zm6.5 0a.9.9 0 0 0 0 1.8h4a.9.9 0 0 0 0-1.8zm-6.5 3.2a.9.9 0 0 0 0 1.8h6.5a.9.9 0 0 0 0-1.8zm9.5 0a.9.9 0 0 0 0 1.8h1a.9.9 0 0 0 0-1.8z"
        />
      ) : (
        <>
          <rect x="3" y="5" width="18" height="14" rx="2.5" />
          <path d="M6.75 13h3.5M13.25 13h4M6.75 16.2h6.5M16.25 16.2h1" />
        </>
      )}
    </svg>
  );
}

export function SlidersIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0" />
      <circle cx="16" cy="6" r="2" />
      <circle cx="10" cy="12" r="2" />
      <circle cx="18" cy="18" r="2" />
    </svg>
  );
}

export function EyeIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  );
}

export function EyeOffIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M10.7 5.1A10 10 0 0 1 12 5c6.4 0 10 7 10 7a17.6 17.6 0 0 1-2.2 3.1M6.6 6.6C3.7 8.6 2 12 2 12s3.6 7 10 7a9.7 9.7 0 0 0 5.4-1.6M9.9 9.9a3 3 0 0 0 4.2 4.2M2 2l20 20" />
    </svg>
  );
}

export function GripIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M9 5h.01M15 5h.01M9 12h.01M15 12h.01M9 19h.01M15 19h.01" strokeWidth={3} />
    </svg>
  );
}

export function ArrowUpIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="m6 11 6-6 6 6M12 5v14" />
    </svg>
  );
}

export function ArrowDownIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="m6 13 6 6 6-6M12 19V5" />
    </svg>
  );
}

export function PencilIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M17 3a2.85 2.85 0 0 1 4 4L7.5 20.5 2 22l1.5-5.5z" />
    </svg>
  );
}

export function TrashIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
    </svg>
  );
}

export function SearchIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.5-3.5" />
    </svg>
  );
}
