import { DEFAULT_DESIGN_SETTINGS, DEFAULT_THEME_ID } from "./defaults";
import { applyDesignSettingsToDocument } from "./dom";
import { themeRegistry } from "./theme-registry";
import type { DesignSettings, Theme } from "./types";

export function resolveTheme(themeId: string): Theme {
  const theme =
    themeRegistry.get(themeId) ??
    themeRegistry.get(DEFAULT_THEME_ID) ??
    themeRegistry.getActiveTheme();
  if (!theme) throw new Error("No active theme is registered");
  return theme;
}

export function applyDesignSystem(
  settings: DesignSettings = DEFAULT_DESIGN_SETTINGS,
): Theme {
  const theme = resolveTheme(settings.themeId);
  themeRegistry.apply(theme.id);
  applyDesignSettingsToDocument(settings, theme);
  return theme;
}
