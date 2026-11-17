import { Outlet, useNavigate } from "@tanstack/react-router";
import { listen } from "@tauri-apps/api/event";
import { type CSSProperties, useEffect } from "react";
import { QueryTabs } from "../features/query/query-tabs";
import { useResizableSidebar } from "../hooks/use-resizable-sidebar";
import {
  isConnectionReady,
  useConnectionsStore,
} from "../stores/connections.store";
import { useSettingsStore } from "../stores/settings.store";
import {
  STATUS_BAR_HEIGHT,
  WINDOW_CHROME_HEIGHT,
  WORKBENCH_HEADER_HEIGHT,
} from "../window/geometry";
import { APP_SHELL_LAYOUT } from "./app-shell-layout";
import { SecretVaultGate } from "./secret-vault-gate";
import { ActivityRail } from "./sidebar/activity-rail";
import { SidebarShell } from "./sidebar/sidebar-shell";
import { StatusBar } from "./status-bar";
import { Toaster, toast } from "./ui/toast";
import { WindowChrome } from "./window-chrome";

type ConnectionHealthEvent = {
  connectionId: string;
  status: "disconnected";
  errorCode: string;
};

export function AppShell() {
  const navigate = useNavigate();
  const sidebarOpen = useSettingsStore((state) => state.sidebarOpen);
  const toggleSidebar = useSettingsStore((state) => state.toggleSidebar);
  const setConnectionStatus = useConnectionsStore(
    (state) => state.setConnectionStatus,
  );
  const resetConnectionTree = useConnectionsStore(
    (state) => state.resetConnectionTree,
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<ConnectionHealthEvent>("connection-health", (event) => {
      const { connectionId } = event.payload;
      const connection =
        useConnectionsStore.getState().connections[connectionId];
      if (!connection || !isConnectionReady(connection.status)) return;
      setConnectionStatus(connectionId, "disconnected");
      resetConnectionTree(connectionId);
      toast.add({
        description: "The database session is no longer available.",
        title: `${connection.name} connection lost`,
        type: "error",
      });
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [resetConnectionTree, setConnectionStatus]);
  const {
    isResizing,
    sidebarWidth,
    startResizing,
    stopResizing,
    resizeWithKeyboard,
  } = useResizableSidebar();
  const mainAreaStyle = {
    "--ds-window-chrome-height": WINDOW_CHROME_HEIGHT,
    "--ds-workbench-header-height": WORKBENCH_HEADER_HEIGHT,
    "--ds-status-bar-height": STATUS_BAR_HEIGHT,
    "--sidebar-column-width": sidebarOpen ? `${sidebarWidth}px` : "0px",
    "--app-navigation-width": sidebarOpen
      ? `calc(3rem + ${sidebarWidth}px)`
      : "3rem",
  } as CSSProperties;

  return (
    <Toaster>
      <div
        style={mainAreaStyle}
        className="app-shell grid h-full w-full min-w-0 grid-rows-[var(--ds-window-chrome-height)_minmax(0,1fr)_var(--ds-status-bar-height)] overflow-hidden bg-app-chrome text-on-surface"
      >
        <WindowChrome />
        <div
          className={`app-surface-region grid h-full min-h-0 w-full min-w-0 ${APP_SHELL_LAYOUT.workspaceColumns} ${isResizing ? "transition-none" : "transition-[grid-template-columns] duration-200 ease-out"}`}
        >
          <div
            className={`app-navigation-surface grid min-h-0 min-w-0 ${sidebarOpen ? APP_SHELL_LAYOUT.navigationColumns : APP_SHELL_LAYOUT.navigationColumnsCollapsed}`}
          >
            <ActivityRail
              explorerOpen={sidebarOpen}
              onToggleExplorer={toggleSidebar}
            />
            {sidebarOpen ? (
              <SidebarShell
                isResizing={isResizing}
                onAddConnection={() => navigate({ to: "/connections/new" })}
                resizeWithKeyboard={resizeWithKeyboard}
                sidebarWidth={sidebarWidth}
                startResizing={startResizing}
                stopResizing={stopResizing}
              />
            ) : null}
          </div>
          <div className="app-workbench-surface grid min-h-0 min-w-0 grid-rows-[32px_minmax(0,1fr)]">
            <header className="flex min-w-0 items-center border-b border-structural-divider bg-app-workspace-subtle pe-3 ps-0">
              <div className="h-full min-w-0 flex-1">
                <QueryTabs />
              </div>
            </header>
            <section className="min-h-0 min-w-0 bg-app-content">
              <main className="h-full min-h-0 min-w-0 overflow-hidden bg-app-content">
                <div className="h-full min-h-0 min-w-0 overflow-auto">
                  <Outlet />
                </div>
              </main>
            </section>
          </div>
        </div>
        <StatusBar />
      </div>
      <SecretVaultGate />
    </Toaster>
  );
}
