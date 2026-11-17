import type { ComponentPropsWithoutRef } from "react";

type OrbitLogoProps = ComponentPropsWithoutRef<"svg"> & {
  size?: number;
};

export function OrbitLogo({ size = 28, ...props }: OrbitLogoProps) {
  return (
    <svg
      aria-hidden="true"
      height={size}
      viewBox="0 0 64 64"
      width={size}
      {...props}
    >
      <circle cx="32" cy="32" fill="var(--ds-color-primary-fixed)" r="28" />
      <ellipse
        cx="32"
        cy="32"
        fill="none"
        rx="23"
        ry="10"
        stroke="var(--ds-color-primary-container)"
        strokeWidth="5"
        transform="rotate(-22 32 32)"
      />
      <ellipse
        cx="32"
        cy="32"
        fill="none"
        rx="23"
        ry="10"
        stroke="var(--ds-color-primary-container)"
        strokeWidth="5"
        transform="rotate(28 32 32)"
      />
      <path
        d="M21 25.5c0-3 4.9-5.5 11-5.5s11 2.5 11 5.5v13c0 3-4.9 5.5-11 5.5s-11-2.5-11-5.5z"
        fill="var(--ds-color-primary-container)"
      />
      <path
        d="M21 25.5c0 3 4.9 5.5 11 5.5s11-2.5 11-5.5"
        fill="none"
        stroke="var(--ds-color-on-primary)"
        strokeWidth="2"
      />
      <circle cx="48" cy="18" fill="var(--ds-color-primary-container)" r="4" />
    </svg>
  );
}
