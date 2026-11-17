export const APP_SHELL_LAYOUT = {
  workspaceColumns:
    "grid-cols-[minmax(0,var(--app-navigation-width))_minmax(0,1fr)]",
  navigationColumns: "grid-cols-[3rem_minmax(0,var(--sidebar-column-width))]",
  navigationColumnsCollapsed: "grid-cols-[3rem]",
  activityBarWidth: "3rem",
  dividerWidth: "1px",
  explorerTrack: "minmax(0,var(--sidebar-column-width))",
  workspaceTrack: "minmax(0,1fr)",
} as const;
