import { describe, expect, it, vi } from "vitest";
import { DEFAULT_THEME_ID } from "./defaults";
import { applyDesignSettingsToDocument, applyThemeToDocument } from "./dom";
import { ThemeRegistry, themeRegistry } from "./theme-registry";
import { precisionGlassStudioTheme } from "./themes/precision-glass-studio";

describe("theme registry", () => {
  it("registers Horizon Lite as the default and keeps legacy presets available", () => {
    expect(themeRegistry.getActiveThemeId()).toBe(DEFAULT_THEME_ID);
    expect(themeRegistry.get(DEFAULT_THEME_ID)?.name).toBe("Horizon Lite");
    expect(themeRegistry.get("dracula")?.name).toBe("Dracula");
    expect(themeRegistry.get("night-owl")?.name).toBe("Night Owl");
    expect(themeRegistry.getAll()).toHaveLength(21);
  });

  it("keeps builtin presets protected while allowing custom themes", () => {
    const registry = new ThemeRegistry([precisionGlassStudioTheme]);
    const customTheme = { ...precisionGlassStudioTheme, id: "custom-test" };

    registry.register(customTheme);

    expect(registry.unregister(precisionGlassStudioTheme.id)).toBe(false);
    expect(registry.unregister("custom-test")).toBe(true);
    expect(registry.getCommunity()).toHaveLength(0);
  });

  it("round trips valid themes and rejects malformed imports", () => {
    const registry = new ThemeRegistry([precisionGlassStudioTheme]);
    const exported = registry.exportTheme(precisionGlassStudioTheme.id);

    expect(exported).not.toBeNull();
    expect(registry.importTheme(exported)).not.toBeNull();
    expect(
      registry.importTheme({ version: 2, theme: precisionGlassStudioTheme }),
    ).toBeNull();
    expect(
      registry.importTheme({ version: 1, theme: { id: "invalid" } }),
    ).toBeNull();
  });

  it("does not apply unknown themes or cancel an inactive preview", () => {
    const registry = new ThemeRegistry([precisionGlassStudioTheme]);

    expect(registry.apply("missing")).toBe(false);
    expect(registry.preview("missing")).toBe(false);
    expect(registry.isPreviewActive()).toBe(false);
    registry.cancelPreview();
    expect(registry.getActiveThemeId()).toBe(DEFAULT_THEME_ID);
  });

  it("writes semantic theme and density tokens to the document root", () => {
    const style = { setProperty: vi.fn() };
    const root = { style, dataset: {} } as unknown as HTMLElement;
    const documentRef = { documentElement: root } as unknown as Document;

    applyThemeToDocument(precisionGlassStudioTheme, documentRef);
    applyDesignSettingsToDocument(
      {
        themeId: DEFAULT_THEME_ID,
        uiFontFamily: "Test UI",
        uiFontSize: 15,
        editorFontFamily: "Test Editor",
        editorFontSize: 16,
        density: "compact",
        radius: 10,
      },
      precisionGlassStudioTheme,
      documentRef,
    );

    expect(root.dataset.theme).toBe(precisionGlassStudioTheme.id);
    expect(style.setProperty).toHaveBeenCalledWith(
      "--ds-color-primary",
      "#0059b5",
    );
    expect(style.setProperty).toHaveBeenCalledWith("--ds-space-4", "12px");
    expect(style.setProperty).toHaveBeenCalledWith(
      "--ds-sidebar-content-inset",
      "var(--ds-space-3)",
    );
    expect(style.setProperty).toHaveBeenCalledWith("--ds-radius-base", "10px");
    expect(style.setProperty).toHaveBeenCalledWith(
      "--ds-font-editor-size",
      "16px",
    );
  });

  it("writes the shared form control and floating surface roles", () => {
    const style = { setProperty: vi.fn() };
    const root = { style, dataset: {} } as unknown as HTMLElement;
    const documentRef = { documentElement: root } as unknown as Document;

    applyThemeToDocument(precisionGlassStudioTheme, documentRef);

    for (const token of [
      "app-chrome",
      "app-chrome-subtle",
      "app-content",
      "app-content-subtle",
      "app-shell",
      "app-sidebar",
      "app-workspace",
      "app-workspace-subtle",
      "app-editor",
      "app-results",
      "app-toolbar",
      "app-statusbar",
      "surface-base",
      "surface-subtle",
      "surface-raised",
      "surface-floating",
      "dialog-surface",
      "dialog-content-surface",
      "dialog-sidebar-surface",
      "surface-overlay",
      "surface-divider",
      "structural-divider",
      "selected-surface",
      "control-surface",
      "control-surface-hover",
      "control-surface-focus",
      "control-surface-active",
      "control-surface-disabled",
      "control-surface-readonly",
      "control-border",
      "control-border-hover",
      "control-border-focus",
      "control-border-disabled",
      "control-border-readonly",
      "control-foreground",
      "control-foreground-disabled",
      "control-placeholder",
      "control-placeholder-disabled",
      "control-icon",
      "control-icon-emphasis",
      "control-focus-ring",
      "db-object-database",
      "db-object-schema",
      "db-object-table",
      "db-object-view",
      "db-object-procedure",
      "db-object-foreign-table",
      "db-object-materialized-view",
      "db-object-index",
      "db-object-function",
      "db-object-sequence",
      "db-object-type",
      "db-object-aggregate",
      "db-object-metadata",
      "floating-surface",
      "floating-border",
    ]) {
      expect(style.setProperty).toHaveBeenCalledWith(
        `--ds-${token}`,
        expect.any(String),
      );
    }

    expect(style.setProperty).toHaveBeenCalledWith(
      "--ds-floating-surface",
      "#ffffff",
    );
    expect(style.setProperty).toHaveBeenCalledWith(
      "--ds-app-chrome",
      "#dad9df",
    );
    expect(style.setProperty).toHaveBeenCalledWith(
      "--ds-app-workspace",
      "#faf8fe",
    );
    expect(style.setProperty).toHaveBeenCalledWith(
      "--ds-control-surface",
      "#f4f3f8",
    );
    expect(style.setProperty).toHaveBeenCalledWith(
      "--ds-control-surface-hover",
      "color-mix(in srgb, #f4f3f8 90%, #1a1b1f 10%)",
    );
  });
});
