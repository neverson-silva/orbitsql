import { DENSITY_MULTIPLIERS } from "./defaults";
import { resolveSurfaceRoles } from "./surfaces";
import type { DesignSettings, Theme } from "./types";

function toKebabCase(value: string): string {
  return value.replace(/[A-Z]/g, (character) => `-${character.toLowerCase()}`);
}

function setToken(root: HTMLElement, name: string, value: string): void {
  root.style.setProperty(`--ds-${name}`, value);
}

function scaleLength(value: string, multiplier: number): string {
  const match = value.trim().match(/^(-?(?:\d+\.?\d*|\.\d+))(px|rem|em)$/);
  if (!match) {
    return value;
  }
  const amount = Number.parseFloat(match[1]) * multiplier;
  const rounded = Math.round(amount * 100) / 100;
  return `${rounded}${match[2]}`;
}

function getDocument(documentRef?: Document): Document | undefined {
  if (documentRef) {
    return documentRef;
  }
  if (typeof document === "undefined") {
    return undefined;
  }
  return document;
}

export function applyThemeToDocument(
  theme: Theme,
  documentRef?: Document,
): void {
  const documentValue = getDocument(documentRef);
  if (!documentValue) {
    return;
  }

  const root = documentValue.documentElement;
  const { colors, typography, radii, spacing, elevation } = theme;

  Object.entries(colors).forEach(([name, value]) => {
    if (typeof value === "string") {
      setToken(root, `color-${toKebabCase(name)}`, value);
    }
  });

  Object.entries(colors.syntax).forEach(([name, value]) => {
    setToken(root, `color-syntax-${toKebabCase(name)}`, value);
  });

  const surfaceRoles = resolveSurfaceRoles(theme);
  const appContent = surfaceRoles.appWorkspace;
  const appChrome = surfaceRoles.appChrome;
  const appChromeSubtle = `color-mix(in srgb, ${appChrome} 88%, ${colors.onSurface} 12%)`;
  const appWorkspaceSubtle = surfaceRoles.appToolbar;
  const surfaceFloating = surfaceRoles.surfaceFloating;
  const dialogSurface = `oklch(from ${appContent} calc(l - 0.035) calc(c * 0.98) h)`;
  const dialogContentSurface = `oklch(from ${appContent} calc(l - 0.025) calc(c * 0.99) h)`;
  const dialogSidebarSurface = `oklch(from ${appContent} calc(l - 0.017) calc(c * 0.99) h)`;
  const controlSurface = surfaceRoles.controlSurface;
  const controlSurfaceHover = `color-mix(in srgb, ${controlSurface} 90%, ${colors.onSurface} 10%)`;
  const controlSurfaceFocus = `color-mix(in srgb, ${controlSurface} 94%, ${colors.primary} 6%)`;
  const controlSurfaceActive = `color-mix(in srgb, ${controlSurface} 88%, ${colors.onSurface} 12%)`;
  const controlSurfaceDisabled = `color-mix(in srgb, ${controlSurface} 90%, ${colors.onSurfaceVariant} 10%)`;
  const controlSurfaceReadonly = `color-mix(in srgb, ${controlSurface} 92%, ${colors.surfaceContainerLow} 8%)`;
  const databaseObjectColors = {
    "db-object-database": `color-mix(in srgb, ${colors.primary} 84%, ${colors.onSurface} 16%)`,
    "db-object-schema": `color-mix(in srgb, ${colors.warning} 62%, ${colors.onSurface} 38%)`,
    "db-object-table": `color-mix(in srgb, ${colors.primary} 72%, ${colors.onSurface} 28%)`,
    "db-object-view": `color-mix(in srgb, ${colors.secondary} 74%, ${colors.onSurface} 26%)`,
    "db-object-procedure": `oklch(from ${colors.secondary} calc(l * 0.92) calc(c * 0.78) calc(h + 16))`,
    "db-object-foreign-table": `oklch(from ${colors.primary} calc(l * 0.92) calc(c * 0.76) calc(h + 78))`,
    "db-object-materialized-view": `oklch(from ${colors.secondary} calc(l * 0.86) calc(c * 0.82) calc(h - 12))`,
    "db-object-index": `color-mix(in srgb, ${colors.warning} 68%, ${colors.onSurface} 32%)`,
    "db-object-function": `color-mix(in srgb, ${colors.secondary} 68%, ${colors.onSurface} 32%)`,
    "db-object-sequence": `oklch(from ${colors.primary} calc(l * 0.9) calc(c * 0.76) calc(h + 60))`,
    "db-object-type": `oklch(from ${colors.secondary} calc(l * 0.9) calc(c * 0.62) calc(h + 30))`,
    "db-object-aggregate": `oklch(from ${colors.secondary} calc(l * 0.84) calc(c * 0.78) calc(h - 22))`,
    "db-object-metadata": `color-mix(in srgb, ${colors.onSurfaceVariant} 78%, ${colors.onSurface} 22%)`,
  };
  const semanticSurfaces = {
    "app-chrome": appChrome,
    "app-chrome-subtle": appChromeSubtle,
    "app-content": appContent,
    "app-content-subtle": appWorkspaceSubtle,
    "app-shell": appChrome,
    "app-sidebar": surfaceRoles.appSidebar,
    "app-workspace": appContent,
    "app-workspace-subtle": appWorkspaceSubtle,
    "app-editor": appContent,
    "app-results": appContent,
    "app-toolbar": surfaceRoles.appToolbar,
    "app-statusbar": appChrome,
    "surface-base": appContent,
    "surface-subtle": appWorkspaceSubtle,
    "surface-raised": surfaceRoles.surfaceRaised,
    "surface-floating": surfaceFloating,
    "dialog-surface": dialogSurface,
    "dialog-content-surface": dialogContentSurface,
    "dialog-sidebar-surface": dialogSidebarSurface,
    "surface-overlay": surfaceFloating,
    "surface-divider": `color-mix(in srgb, ${colors.outlineVariant} 72%, ${colors.outline} 28%)`,
    "structural-divider": `color-mix(in srgb, ${colors.outline} 90%, ${colors.onSurface} 10%)`,
    "selected-surface": `color-mix(in srgb, ${colors.surfaceContainerHigh} 82%, ${colors.primaryContainer} 18%)`,
    "control-surface": controlSurface,
    "control-surface-hover": controlSurfaceHover,
    "control-surface-focus": controlSurfaceFocus,
    "control-surface-active": controlSurfaceActive,
    "control-surface-disabled": controlSurfaceDisabled,
    "control-surface-readonly": controlSurfaceReadonly,
    "control-border": `color-mix(in srgb, ${colors.outline} 72%, ${colors.onSurface} 28%)`,
    "control-border-hover": colors.outline,
    "control-border-focus": colors.primary,
    "control-border-disabled": `color-mix(in srgb, ${colors.outlineVariant} 60%, ${colors.surfaceContainerHigh} 40%)`,
    "control-border-readonly": `color-mix(in srgb, ${colors.outlineVariant} 82%, ${colors.outline} 18%)`,
    "control-foreground": colors.onSurface,
    "control-foreground-disabled": `color-mix(in srgb, ${colors.onSurfaceVariant} 68%, ${colors.surfaceContainerHigh} 32%)`,
    "control-placeholder": `color-mix(in srgb, ${colors.onSurfaceVariant} 86%, ${colors.surfaceContainerHigh} 14%)`,
    "control-placeholder-disabled": `color-mix(in srgb, ${colors.onSurfaceVariant} 54%, ${colors.surfaceContainerHigh} 46%)`,
    "control-icon": `color-mix(in srgb, ${colors.onSurfaceVariant} 70%, ${colors.onSurface} 30%)`,
    "control-icon-emphasis": colors.onSurfaceVariant,
    "control-focus-ring": `color-mix(in srgb, ${colors.primary} 78%, ${colors.onSurface} 22%)`,
    "floating-surface": surfaceFloating,
    "floating-border": `color-mix(in srgb, ${colors.outline} 78%, ${colors.onSurface} 22%)`,
    "overlay-backdrop": `color-mix(in srgb, ${colors.surfaceDim} 24%, transparent)`,
  };
  Object.entries(semanticSurfaces).forEach(([name, value]) => {
    setToken(root, name, value);
  });
  Object.entries(databaseObjectColors).forEach(([name, value]) => {
    setToken(root, name, value);
  });

  Object.entries(typography).forEach(([name, token]) => {
    const tokenName = `type-${toKebabCase(name)}`;
    setToken(root, `${tokenName}-font-family`, token.fontFamily);
    setToken(root, `${tokenName}-font-size`, `${token.fontSize}px`);
    setToken(root, `${tokenName}-font-weight`, `${token.fontWeight}`);
    setToken(root, `${tokenName}-line-height`, `${token.lineHeight}px`);
    setToken(root, `${tokenName}-letter-spacing`, token.letterSpacing);
  });

  Object.entries(radii).forEach(([name, value]) => {
    setToken(root, `radius-${toKebabCase(name)}`, value);
  });

  Object.entries(spacing).forEach(([name, value]) => {
    setToken(root, `space-${toKebabCase(name)}`, value);
  });

  Object.entries(elevation).forEach(([name, value]) => {
    setToken(root, `elevation-${toKebabCase(name)}`, value);
  });

  root.dataset.theme = theme.id;
  root.dataset.themeMode = theme.type;
  root.style.setProperty("color-scheme", theme.type);
}

export function applyDesignSettingsToDocument(
  settings: DesignSettings,
  theme: Theme,
  documentRef?: Document,
): void {
  const documentValue = getDocument(documentRef);
  if (!documentValue) return;

  const root = documentValue.documentElement;
  const densityMultiplier = DENSITY_MULTIPLIERS[settings.density];
  const uiBaseSize = theme.typography["body-md"].fontSize;
  const editorBaseSize = theme.typography["code-md"].fontSize;
  const uiScale = settings.uiFontSize / uiBaseSize;
  const editorScale = settings.editorFontSize / editorBaseSize;

  setToken(root, "font-ui", settings.uiFontFamily);
  setToken(root, "font-editor", settings.editorFontFamily);
  setToken(root, "font-ui-size", `${settings.uiFontSize}px`);
  setToken(root, "font-editor-size", `${settings.editorFontSize}px`);
  setToken(root, "density", settings.density);
  setToken(root, "density-multiplier", `${densityMultiplier}`);
  setToken(root, "spacing-unit", `${4 * densityMultiplier}px`);

  for (let index = 0; index <= 12; index += 1) {
    setToken(root, `space-${index}`, `${4 * index * densityMultiplier}px`);
  }

  const scaledSpacing = Object.entries(theme.spacing);
  scaledSpacing.forEach(([name, value]) => {
    setToken(
      root,
      `space-${toKebabCase(name)}`,
      scaleLength(value, densityMultiplier),
    );
  });
  setToken(root, "sidebar-content-inset", "var(--ds-space-3)");

  const radius = settings.radius;
  setToken(root, "radius-none", "0px");
  setToken(root, "radius-sm", `${Math.round(radius * 0.5)}px`);
  setToken(root, "radius-base", `${radius}px`);
  setToken(root, "radius-md", `${Math.round(radius * 1.5)}px`);
  setToken(root, "radius-lg", `${Math.round(radius * 2)}px`);
  setToken(root, "radius-xl", `${Math.round(radius * 3)}px`);
  setToken(root, "radius-full", "9999px");

  Object.entries(theme.typography).forEach(([name, token]) => {
    const tokenName = `type-${toKebabCase(name)}`;
    const scale = name.startsWith("code-") ? editorScale : uiScale;
    const fontFamily = name.startsWith("code-")
      ? settings.editorFontFamily
      : settings.uiFontFamily;
    setToken(root, `${tokenName}-font-family`, fontFamily);
    setToken(
      root,
      `${tokenName}-font-size`,
      `${Math.round(token.fontSize * scale * 100) / 100}px`,
    );
    setToken(
      root,
      `${tokenName}-line-height`,
      `${Math.round(token.lineHeight * scale * 100) / 100}px`,
    );
  });
}
