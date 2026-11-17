export {
  createCodeMirrorTheme,
  createCodeMirrorThemeForId,
  themeCompartment,
} from "./codemirror";
export {
  DEFAULT_DESIGN_SETTINGS,
  DEFAULT_THEME_ID,
  DENSITY_MULTIPLIERS,
  DENSITY_OPTIONS,
  EDITOR_FONT_FAMILIES,
  EDITOR_FONT_SIZE_RANGE,
  UI_FONT_FAMILIES,
  UI_FONT_SIZE_RANGE,
} from "./defaults";
export { applyDesignSettingsToDocument, applyThemeToDocument } from "./dom";
export { applyDesignSystem, resolveTheme } from "./engine";
export {
  precisionElevation,
  precisionRadii,
  precisionSpacing,
  precisionTypography,
} from "./precision";
export type {
  SurfaceDiagnosticRow,
  SurfaceRoleName,
  ThemeSurfaceRoles,
} from "./surfaces";
export {
  formatSurfaceMetric,
  getSurfaceDiagnosticRows,
  resolveSurfaceRoles,
} from "./surfaces";
export {
  ThemeRegistry,
  themeRegistry,
} from "./theme-registry";
export { legacyThemes } from "./themes/legacy";
export { precisionGlassStudioTheme } from "./themes/precision-glass-studio";
export type {
  Density,
  DesignSettings,
  Theme,
  ThemeColors,
  ThemeElevation,
  ThemeExport,
  ThemeMode,
  ThemeRadii,
  ThemeSpacing,
  ThemeSyntax,
  ThemeTypography,
  TypographyToken,
  TypographyTokenName,
} from "./types";
