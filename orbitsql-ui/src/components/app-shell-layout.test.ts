import { describe, expect, it } from "vitest";
import { APP_SHELL_LAYOUT } from "./app-shell-layout";

describe("app shell layout", () => {
  it("keeps the activity boundary fixed while the explorer owns its track", () => {
    expect(APP_SHELL_LAYOUT).toEqual({
      workspaceColumns:
        "grid-cols-[minmax(0,var(--app-navigation-width))_minmax(0,1fr)]",
      navigationColumns:
        "grid-cols-[3rem_minmax(0,var(--sidebar-column-width))]",
      navigationColumnsCollapsed: "grid-cols-[3rem]",
      activityBarWidth: "3rem",
      dividerWidth: "1px",
      explorerTrack: "minmax(0,var(--sidebar-column-width))",
      workspaceTrack: "minmax(0,1fr)",
    });
  });
});
