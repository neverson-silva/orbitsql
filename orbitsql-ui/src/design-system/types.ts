export type ThemeMode = "light" | "dark";

export type Density = "compact" | "normal" | "comfortable";

export type TypographyTokenName =
  | "display-lg"
  | "headline-md"
  | "headline-sm"
  | "body-lg"
  | "body-md"
  | "body-sm"
  | "code-lg"
  | "code-md"
  | "code-sm"
  | "label-md"
  | "label-caps";

export type TypographyToken = {
  fontFamily: string;
  fontSize: number;
  fontWeight: number;
  lineHeight: number;
  letterSpacing: string;
};

export type ThemeTypography = Record<TypographyTokenName, TypographyToken>;

export type ThemeSyntax = {
  keyword: string;
  string: string;
  number: string;
  comment: string;
  function: string;
  variable: string;
  operator: string;
  type: string;
  table: string;
};

export type ThemeColors = {
  surface: string;
  surfaceDim: string;
  surfaceBright: string;
  surfaceContainerLowest: string;
  surfaceContainerLow: string;
  surfaceContainer: string;
  surfaceContainerHigh: string;
  surfaceContainerHighest: string;
  onSurface: string;
  onSurfaceVariant: string;
  inverseSurface: string;
  inverseOnSurface: string;
  outline: string;
  outlineVariant: string;
  surfaceTint: string;
  primary: string;
  onPrimary: string;
  primaryContainer: string;
  onPrimaryContainer: string;
  inversePrimary: string;
  secondary: string;
  onSecondary: string;
  secondaryContainer: string;
  onSecondaryContainer: string;
  tertiary: string;
  onTertiary: string;
  tertiaryContainer: string;
  onTertiaryContainer: string;
  error: string;
  onError: string;
  errorContainer: string;
  onErrorContainer: string;
  success: string;
  onSuccess: string;
  successContainer: string;
  onSuccessContainer: string;
  warning: string;
  onWarning: string;
  warningContainer: string;
  onWarningContainer: string;
  primaryFixed: string;
  primaryFixedDim: string;
  onPrimaryFixed: string;
  onPrimaryFixedVariant: string;
  secondaryFixed: string;
  secondaryFixedDim: string;
  onSecondaryFixed: string;
  onSecondaryFixedVariant: string;
  tertiaryFixed: string;
  tertiaryFixedDim: string;
  onTertiaryFixed: string;
  onTertiaryFixedVariant: string;
  background: string;
  onBackground: string;
  surfaceVariant: string;
  syntax: ThemeSyntax;
};

export type ThemeRadii = {
  none: string;
  sm: string;
  base: string;
  md: string;
  lg: string;
  xl: string;
  full: string;
};

export type ThemeSpacing = {
  gutter: string;
  gutterDense: string;
  margin: string;
  marginCompact: string;
  spaceXs: string;
  spaceSm: string;
  spaceMd: string;
  spaceLg: string;
  spaceXl: string;
};

export type ThemeElevation = {
  panel: string;
  floating: string;
  overlay: string;
};

export type Theme = {
  id: string;
  name: string;
  author: string;
  type: ThemeMode;
  colors: ThemeColors;
  typography: ThemeTypography;
  radii: ThemeRadii;
  spacing: ThemeSpacing;
  elevation: ThemeElevation;
};

export type ThemeExport = {
  version: 1;
  theme: Theme;
};

export type DesignSettings = {
  themeId: string;
  uiFontFamily: string;
  uiFontSize: number;
  editorFontFamily: string;
  editorFontSize: number;
  density: Density;
  radius: number;
};
