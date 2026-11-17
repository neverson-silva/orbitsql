import type { Density, DesignSettings } from "./types";

export const DEFAULT_THEME_ID = "light";

export const DEFAULT_DESIGN_SETTINGS: Readonly<DesignSettings> = {
  themeId: DEFAULT_THEME_ID,
  uiFontFamily: "Inter, ui-sans-serif, system-ui, sans-serif",
  uiFontSize: 13,
  editorFontFamily: '"JetBrains Mono", ui-monospace, SFMono-Regular, monospace',
  editorFontSize: 12,
  density: "normal",
  radius: 8,
};

export const DENSITY_MULTIPLIERS: Readonly<Record<Density, number>> = {
  compact: 0.75,
  normal: 1,
  comfortable: 1.25,
};

export const UI_FONT_FAMILIES = [
  {
    id: "inter",
    name: "Inter",
    value: "Inter, ui-sans-serif, system-ui, sans-serif",
  },
  {
    id: "system",
    name: "System UI",
    value: "ui-sans-serif, system-ui, sans-serif",
  },
  {
    id: "sf-pro",
    name: "SF Pro",
    value: "-apple-system, BlinkMacSystemFont, ui-sans-serif, sans-serif",
  },
] as const;

export const EDITOR_FONT_FAMILIES = [
  {
    id: "jetbrains-mono",
    name: "JetBrains Mono",
    value: '"JetBrains Mono", ui-monospace, SFMono-Regular, monospace',
  },
  {
    id: "fira-code",
    name: "Fira Code",
    value: '"Fira Code", ui-monospace, SFMono-Regular, monospace',
  },
  {
    id: "sf-mono",
    name: "SF Mono",
    value: '"SF Mono", ui-monospace, SFMono-Regular, monospace',
  },
  {
    id: "system-mono",
    name: "System Mono",
    value: "ui-monospace, SFMono-Regular, Menlo, monospace",
  },
] as const;

export const UI_FONT_SIZE_RANGE = { min: 11, max: 20, step: 1 } as const;
export const EDITOR_FONT_SIZE_RANGE = { min: 10, max: 24, step: 1 } as const;

export const DENSITY_OPTIONS = [
  { id: "compact", name: "Compact", multiplier: 0.75 },
  { id: "normal", name: "Normal", multiplier: 1 },
  { id: "comfortable", name: "Comfortable", multiplier: 1.25 },
] as const;
