import type { SVGProps } from "react";

/* The web app's own icons (apps/web/src/components/Icon.tsx), so the illustrations show the real controls. */

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

export function PauseIcon({ size = 18, ...rest }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden focusable={false} {...rest}>
      <rect x="5.5" y="4" width="4.5" height="16" rx="1.2" fill="currentColor" />
      <rect x="14" y="4" width="4.5" height="16" rx="1.2" fill="currentColor" />
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

export function SearchIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.5-3.5" />
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

export function UsersIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
      <circle cx="9" cy="7" r="4" />
      <path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" />
    </svg>
  );
}

export function CheckIcon(p: IconProps) {
  return (
    <svg {...stroke({ strokeWidth: 2.25, ...p })}>
      <path d="M20 6 9 17l-5-5" />
    </svg>
  );
}

export function ChevronDownIcon(p: IconProps) {
  return (
    <svg {...stroke(p)}>
      <path d="m6 9 6 6 6-6" />
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

function Skip10({ forward, ...p }: IconProps & { forward: boolean }) {
  return (
    <svg {...stroke({ size: 28, strokeWidth: 1.9, ...p })}>
      <g transform={forward ? "matrix(-1 0 0 1 24 0)" : undefined}>
        <path d="M4.2 8.5A8.8 8.8 0 1 1 3.4 13" />
        <path d="M3.6 3.8v4.9h4.9" />
      </g>
      <text x="12.4" y="15.6" textAnchor="middle" fontSize="7.4" fontWeight="700" fill="currentColor" stroke="none">
        10
      </text>
    </svg>
  );
}

export const Back10Icon = (p: IconProps) => <Skip10 forward={false} {...p} />;
export const Forward10Icon = (p: IconProps) => <Skip10 forward {...p} />;

export function VolumeIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 22, ...p })}>
      <path
        d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6A1.4 1.4 0 0 1 5.4 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.4a1.4 1.4 0 0 1 1 .4l3.4 3.4a.7.7 0 0 0 1.2-.5z"
        fill="currentColor"
      />
      <path d="M15.5 8.5a5 5 0 0 1 0 7" />
      <path d="M18.7 5.3a9.5 9.5 0 0 1 0 13.4" />
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

export function FullscreenIcon(p: IconProps) {
  return (
    <svg {...stroke({ size: 22, strokeWidth: 2, ...p })}>
      <path d="M8 3H5a2 2 0 0 0-2 2v3M21 8V5a2 2 0 0 0-2-2h-3M3 16v3a2 2 0 0 0 2 2h3M16 21h3a2 2 0 0 0 2-2v-3" />
    </svg>
  );
}

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
