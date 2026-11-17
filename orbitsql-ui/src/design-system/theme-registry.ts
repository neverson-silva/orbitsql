import { DEFAULT_THEME_ID } from "./defaults";
import { applyThemeToDocument } from "./dom";
import { legacyThemes } from "./themes/legacy";
import { precisionGlassStudioTheme } from "./themes/precision-glass-studio";
import type {
  Theme,
  ThemeColors,
  ThemeElevation,
  ThemeExport,
  ThemeRadii,
  ThemeSpacing,
  ThemeSyntax,
  ThemeTypography,
} from "./types";

const BUILTIN_THEMES: Theme[] = [
  precisionGlassStudioTheme,
  ...legacyThemes.filter(
    (theme) => theme.id !== "dark" && theme.id !== "light",
  ),
  ...legacyThemes.filter(
    (theme) => theme.id === "dark" || theme.id === "light",
  ),
];

const COLOR_KEYS: readonly (keyof Omit<ThemeColors, "syntax">)[] = [
  "surface",
  "surfaceDim",
  "surfaceBright",
  "surfaceContainerLowest",
  "surfaceContainerLow",
  "surfaceContainer",
  "surfaceContainerHigh",
  "surfaceContainerHighest",
  "onSurface",
  "onSurfaceVariant",
  "inverseSurface",
  "inverseOnSurface",
  "outline",
  "outlineVariant",
  "surfaceTint",
  "primary",
  "onPrimary",
  "primaryContainer",
  "onPrimaryContainer",
  "inversePrimary",
  "secondary",
  "onSecondary",
  "secondaryContainer",
  "onSecondaryContainer",
  "tertiary",
  "onTertiary",
  "tertiaryContainer",
  "onTertiaryContainer",
  "error",
  "onError",
  "errorContainer",
  "onErrorContainer",
  "success",
  "onSuccess",
  "successContainer",
  "onSuccessContainer",
  "warning",
  "onWarning",
  "warningContainer",
  "onWarningContainer",
  "primaryFixed",
  "primaryFixedDim",
  "onPrimaryFixed",
  "onPrimaryFixedVariant",
  "secondaryFixed",
  "secondaryFixedDim",
  "onSecondaryFixed",
  "onSecondaryFixedVariant",
  "tertiaryFixed",
  "tertiaryFixedDim",
  "onTertiaryFixed",
  "onTertiaryFixedVariant",
  "background",
  "onBackground",
  "surfaceVariant",
];

const SYNTAX_KEYS: readonly (keyof ThemeSyntax)[] = [
  "keyword",
  "string",
  "number",
  "comment",
  "function",
  "variable",
  "operator",
  "type",
  "table",
];

const TYPOGRAPHY_KEYS: readonly (keyof ThemeTypography)[] = [
  "display-lg",
  "headline-md",
  "headline-sm",
  "body-lg",
  "body-md",
  "body-sm",
  "code-lg",
  "code-md",
  "code-sm",
  "label-md",
  "label-caps",
];

const RADII_KEYS: readonly (keyof ThemeRadii)[] = [
  "none",
  "sm",
  "base",
  "md",
  "lg",
  "xl",
  "full",
];

const SPACING_KEYS: readonly (keyof ThemeSpacing)[] = [
  "gutter",
  "gutterDense",
  "margin",
  "marginCompact",
  "spaceXs",
  "spaceSm",
  "spaceMd",
  "spaceLg",
  "spaceXl",
];

const ELEVATION_KEYS: readonly (keyof ThemeElevation)[] = [
  "panel",
  "floating",
  "overlay",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function hasStringFields(
  value: unknown,
  keys: readonly string[],
): value is Record<string, string> {
  return isRecord(value) && keys.every((key) => typeof value[key] === "string");
}

function hasTypographyFields(value: unknown): value is ThemeTypography {
  if (!isRecord(value)) return false;
  return TYPOGRAPHY_KEYS.every((key) => {
    const token = value[key];
    return (
      isRecord(token) &&
      typeof token.fontFamily === "string" &&
      typeof token.fontSize === "number" &&
      typeof token.fontWeight === "number" &&
      typeof token.lineHeight === "number" &&
      typeof token.letterSpacing === "string"
    );
  });
}

export class ThemeRegistry {
  private readonly themes = new Map<string, Theme>();
  private readonly builtinThemeIds = new Set<string>();
  private activeThemeId = DEFAULT_THEME_ID;
  private previewThemeId: string | null = null;

  constructor(themes: readonly Theme[] = BUILTIN_THEMES) {
    themes.forEach((theme) => {
      this.register(theme);
      this.builtinThemeIds.add(theme.id);
    });
  }

  register(theme: Theme): void {
    if (!this.validateTheme(theme)) {
      throw new TypeError("Invalid theme");
    }
    this.themes.set(theme.id, theme);
  }

  unregister(themeId: string): boolean {
    if (this.builtinThemeIds.has(themeId)) return false;
    return this.themes.delete(themeId);
  }

  get(themeId: string): Theme | undefined {
    return this.themes.get(themeId);
  }

  getAll(): Theme[] {
    return Array.from(this.themes.values());
  }

  getBuiltin(): Theme[] {
    return Array.from(this.builtinThemeIds)
      .map((themeId) => this.themes.get(themeId))
      .filter((theme): theme is Theme => theme !== undefined);
  }

  getCommunity(): Theme[] {
    return this.getAll().filter((theme) => !this.builtinThemeIds.has(theme.id));
  }

  apply(themeId: string): boolean {
    const theme = this.themes.get(themeId);
    if (!theme) return false;
    this.activeThemeId = themeId;
    this.previewThemeId = null;
    applyThemeToDocument(theme);
    return true;
  }

  getActiveTheme(): Theme | undefined {
    return this.themes.get(this.activeThemeId);
  }

  getActiveThemeId(): string {
    return this.activeThemeId;
  }

  preview(themeId: string): boolean {
    const theme = this.themes.get(themeId);
    if (!theme) return false;
    this.previewThemeId = themeId;
    applyThemeToDocument(theme);
    return true;
  }

  cancelPreview(): void {
    if (this.previewThemeId === null) return;
    this.previewThemeId = null;
    const activeTheme = this.getActiveTheme();
    if (activeTheme) applyThemeToDocument(activeTheme);
  }

  isPreviewActive(): boolean {
    return this.previewThemeId !== null;
  }

  exportTheme(themeId: string): ThemeExport | null {
    const theme = this.themes.get(themeId);
    if (!theme) return null;
    return { version: 1, theme };
  }

  importTheme(data: unknown): Theme | null {
    if (
      !isRecord(data) ||
      data.version !== 1 ||
      !this.validateTheme(data.theme)
    ) {
      return null;
    }

    const theme = { ...data.theme, id: `custom-${Date.now()}` };
    this.register(theme);
    return theme;
  }

  validateTheme(theme: unknown): theme is Theme {
    if (!isRecord(theme)) return false;
    if (
      typeof theme.id !== "string" ||
      typeof theme.name !== "string" ||
      typeof theme.author !== "string" ||
      (theme.type !== "light" && theme.type !== "dark")
    ) {
      return false;
    }

    const colors = theme.colors;
    if (
      !hasStringFields(colors, COLOR_KEYS) ||
      !hasStringFields(
        isRecord(colors) ? colors.syntax : undefined,
        SYNTAX_KEYS,
      )
    ) {
      return false;
    }

    return (
      hasTypographyFields(theme.typography) &&
      hasStringFields(theme.radii, RADII_KEYS) &&
      hasStringFields(theme.spacing, SPACING_KEYS) &&
      hasStringFields(theme.elevation, ELEVATION_KEYS)
    );
  }
}

export const themeRegistry = new ThemeRegistry();
