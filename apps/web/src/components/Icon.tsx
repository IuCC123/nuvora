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
