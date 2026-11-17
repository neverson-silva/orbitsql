import type { Theme, ThemeColors } from "./types";

export type SurfaceRoleName =
  | "appChrome"
  | "appSidebar"
  | "appWorkspace"
  | "appToolbar"
  | "surfaceRaised"
  | "surfaceFloating"
  | "controlSurface";

export type ThemeSurfaceRoles = Record<SurfaceRoleName, string>;

const paletteKeys = [
  "surfaceDim",
  "surfaceContainerLowest",
  "surfaceContainerLow",
  "background",
  "surface",
  "surfaceContainer",
  "surfaceContainerHigh",
  "surfaceContainerHighest",
  "surfaceBright",
  "surfaceVariant",
] as const satisfies readonly (keyof ThemeColors)[];

type PaletteSurfaceRole = (typeof paletteKeys)[number];

export type SurfaceDiagnosticRow = {
  role: SurfaceRoleName | PaletteSurfaceRole;
  color: string;
  oklchLightness: number | null;
  relativeLuminance: number | null;
};

function srgbToLinear(value: number): number {
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

function parseHexColor(value: string): [number, number, number] | null {
  const normalized = value.trim().replace(/^#/, "");
  const expanded =
    normalized.length === 3
      ? normalized
          .split("")
          .map((part) => `${part}${part}`)
          .join("")
      : normalized;
  if (!/^[0-9a-f]{6}$/i.test(expanded)) return null;
  return [0, 2, 4].map((index) =>
    srgbToLinear(Number.parseInt(expanded.slice(index, index + 2), 16) / 255),
  ) as [number, number, number];
}

function colorMetrics(value: string): {
  oklchLightness: number;
  relativeLuminance: number;
} | null {
  const rgb = parseHexColor(value);
  if (!rgb) return null;
  const [red, green, blue] = rgb;
  const relativeLuminance = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
  const l = 0.4122214708 * red + 0.5363325363 * green + 0.0514459929 * blue;
  const m = 0.2119034982 * red + 0.6806995451 * green + 0.1073969566 * blue;
  const s = 0.0883024619 * red + 0.2817188376 * green + 0.6299787005 * blue;
  const lRoot = Math.cbrt(l);
  const mRoot = Math.cbrt(m);
  const sRoot = Math.cbrt(s);
  const lightness =
    0.2104542553 * lRoot + 0.793617785 * mRoot - 0.0040720468 * sRoot;
  return { oklchLightness: lightness, relativeLuminance };
}

export function resolveSurfaceRoles(theme: Theme): ThemeSurfaceRoles {
  const { colors } = theme;
  const lightTheme = theme.type === "light";
  return {
    appChrome: colors.surfaceDim,
    appSidebar: colors.surfaceContainerLow,
    appWorkspace: colors.background,
    appToolbar: colors.surfaceContainer,
    surfaceRaised: lightTheme
      ? colors.surfaceContainerLowest
      : colors.surfaceContainerHigh,
    surfaceFloating: lightTheme
      ? colors.surfaceContainerLowest
      : colors.surfaceContainerHighest,
    controlSurface: lightTheme
      ? colors.surfaceContainerLow
      : colors.surfaceContainerHigh,
  };
}

export function getSurfaceDiagnosticRows(theme: Theme): SurfaceDiagnosticRow[] {
  const roles = resolveSurfaceRoles(theme);
  const rows: SurfaceDiagnosticRow[] = [
    ...Object.entries(roles).map(([role, color]) => ({
      role: role as SurfaceRoleName,
      color,
      ...((metrics) =>
        metrics
          ? {
              oklchLightness: metrics.oklchLightness,
              relativeLuminance: metrics.relativeLuminance,
            }
          : { oklchLightness: null, relativeLuminance: null })(
        colorMetrics(color),
      ),
    })),
  ];
  const paletteRows = paletteKeys.map((role) => {
    const color = theme.colors[role];
    const metrics = colorMetrics(color);
    return {
      role,
      color,
      oklchLightness: metrics?.oklchLightness ?? null,
      relativeLuminance: metrics?.relativeLuminance ?? null,
    };
  });
  return [...rows, ...paletteRows];
}

export function formatSurfaceMetric(value: number | null): string {
  return value === null ? "n/a" : value.toFixed(3);
}
