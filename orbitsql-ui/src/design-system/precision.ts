import type {
  ThemeElevation,
  ThemeRadii,
  ThemeSpacing,
  ThemeTypography,
} from "./types";

export const precisionTypography: ThemeTypography = {
  "display-lg": {
    fontFamily: "Inter",
    fontSize: 32,
    fontWeight: 600,
    lineHeight: 40,
    letterSpacing: "-0.025em",
  },
  "headline-md": {
    fontFamily: "Inter",
    fontSize: 22,
    fontWeight: 600,
    lineHeight: 28,
    letterSpacing: "-0.015em",
  },
  "headline-sm": {
    fontFamily: "Inter",
    fontSize: 17,
    fontWeight: 600,
    lineHeight: 22,
    letterSpacing: "-0.01em",
  },
  "body-lg": {
    fontFamily: "Inter",
    fontSize: 15,
    fontWeight: 400,
    lineHeight: 22,
    letterSpacing: "-0.005em",
  },
  "body-md": {
    fontFamily: "Inter",
    fontSize: 13,
    fontWeight: 400,
    lineHeight: 18,
    letterSpacing: "0em",
  },
  "body-sm": {
    fontFamily: "Inter",
    fontSize: 12,
    fontWeight: 400,
    lineHeight: 16,
    letterSpacing: "0em",
  },
  "code-lg": {
    fontFamily: "JetBrains Mono",
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 22,
    letterSpacing: "-0.01em",
  },
  "code-md": {
    fontFamily: "JetBrains Mono",
    fontSize: 12,
    fontWeight: 400,
    lineHeight: 18,
    letterSpacing: "0em",
  },
  "code-sm": {
    fontFamily: "JetBrains Mono",
    fontSize: 11,
    fontWeight: 400,
    lineHeight: 16,
    letterSpacing: "0.01em",
  },
  "label-md": {
    fontFamily: "Inter",
    fontSize: 11,
    fontWeight: 600,
    lineHeight: 14,
    letterSpacing: "0.04em",
  },
  "label-caps": {
    fontFamily: "Inter",
    fontSize: 10,
    fontWeight: 600,
    lineHeight: 12,
    letterSpacing: "0.06em",
  },
};

export const precisionRadii: ThemeRadii = {
  none: "0px",
  sm: "4px",
  base: "8px",
  md: "12px",
  lg: "16px",
  xl: "24px",
  full: "9999px",
};

export const precisionSpacing: ThemeSpacing = {
  gutter: "16px",
  gutterDense: "8px",
  margin: "16px",
  marginCompact: "8px",
  spaceXs: "4px",
  spaceSm: "8px",
  spaceMd: "12px",
  spaceLg: "20px",
  spaceXl: "32px",
};

export const precisionElevation: ThemeElevation = {
  panel: "0 1px 2px rgba(0, 0, 0, 0.04), 0 4px 12px rgba(0, 0, 0, 0.02)",
  floating: "0 4px 20px rgba(0, 0, 0, 0.06)",
  overlay:
    "0 12px 32px -4px rgba(0, 0, 0, 0.12), 0 4px 12px -2px rgba(0, 0, 0, 0.06)",
};
