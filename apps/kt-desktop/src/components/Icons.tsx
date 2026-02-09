import { type SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

// Base icon wrapper with sensible defaults
function Icon({ className = "w-4 h-4", "aria-hidden": ariaHidden = true, ...props }: IconProps) {
  return (
    <svg
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      viewBox="0 0 24 24"
      className={className}
      aria-hidden={ariaHidden}
      {...props}
    />
  );
}

// ─────────────────────────────────────────────────────────────
// Action Icons
// ─────────────────────────────────────────────────────────────

export function PlusIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 4v16m8-8H4" />
    </Icon>
  );
}

export function XIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M6 18L18 6M6 6l12 12" />
    </Icon>
  );
}

export function MenuIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M4 6h16M4 12h16M4 18h16" />
    </Icon>
  );
}

// ─────────────────────────────────────────────────────────────
// UI/Feature Icons
// ─────────────────────────────────────────────────────────────

export function TerminalIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
    </Icon>
  );
}

export function DocumentIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
    </Icon>
  );
}

// ─────────────────────────────────────────────────────────────
// Status/Alert Icons
// ─────────────────────────────────────────────────────────────

export function CheckIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M5 13l4 4L19 7" />
    </Icon>
  );
}

export function AlertTriangleIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
    </Icon>
  );
}

export function InfoCircleIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
    </Icon>
  );
}

// ─────────────────────────────────────────────────────────────
// Topology/Network Icons
// ─────────────────────────────────────────────────────────────

export function OrchestratorIcon(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 5v2M12 17v2M5 12h2M17 12h2M7.05 7.05l1.41 1.41M15.54 15.54l1.41 1.41M7.05 16.95l1.41-1.41M15.54 8.46l1.41-1.41" />
    </Icon>
  );
}

// ─────────────────────────────────────────────────────────────
// Icon path data for dynamic rendering (Toast component)
// ─────────────────────────────────────────────────────────────

export const iconPaths = {
  success: "M5 13l4 4L19 7",
  error: "M6 18L18 6M6 6l12 12",
  warning:
    "M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z",
  info: "M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z",
  close: "M6 18L18 6M6 6l12 12",
} as const;

export type IconPathKey = keyof typeof iconPaths;
