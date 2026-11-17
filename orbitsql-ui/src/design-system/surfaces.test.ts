import { describe, expect, it } from "vitest";
import { getSurfaceDiagnosticRows, resolveSurfaceRoles } from "./surfaces";
import { legacyThemes } from "./themes/legacy";
import { precisionGlassStudioTheme } from "./themes/precision-glass-studio";

const themes = [precisionGlassStudioTheme, ...legacyThemes];

describe("theme surface roles", () => {
  it("maps structural roles to declared palette materials", () => {
    for (const theme of themes) {
      const roles = resolveSurfaceRoles(theme);

      expect(roles.appChrome).toBe(theme.colors.surfaceDim);
      expect(roles.appWorkspace).toBe(theme.colors.background);
      expect(roles.appToolbar).toBe(theme.colors.surfaceContainer);
      expect(roles.appSidebar).toBe(theme.colors.surfaceContainerLow);
      expect(roles.controlSurface).toBe(
        theme.type === "light"
          ? theme.colors.surfaceContainerLow
          : theme.colors.surfaceContainerHigh,
      );
    }
  });

  it("keeps the editor and result work plane stable while allowing raised depth", () => {
    for (const theme of themes) {
      const roles = resolveSurfaceRoles(theme);
      expect(roles.appWorkspace).toBe(theme.colors.background);
      expect(roles.surfaceRaised).toBe(
        theme.type === "light"
          ? theme.colors.surfaceContainerLowest
          : theme.colors.surfaceContainerHigh,
      );
      expect(roles.surfaceFloating).toBe(
        theme.type === "light"
          ? theme.colors.surfaceContainerLowest
          : theme.colors.surfaceContainerHighest,
      );
    }
  });

  it("reports numeric diagnostics for every bundled surface material", () => {
    for (const theme of themes) {
      const rows = getSurfaceDiagnosticRows(theme);
      expect(rows).toHaveLength(17);
      expect(rows.every((row) => row.oklchLightness !== null)).toBe(true);
      expect(rows.every((row) => row.relativeLuminance !== null)).toBe(true);
    }
  });
});
