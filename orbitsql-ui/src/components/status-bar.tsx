import { APP_NAME, APP_VERSION_LABEL } from "../app-meta";
import {
  dialectForConnection,
  formatExecutionContext,
  getExecutionContextCapabilities,
} from "../features/connections/execution-context";
import {
  type ConnectionStatus,
  isConnectionReady,
  useConnectionsStore,
} from "../stores/connections.store";
import { useQueryTabsStore } from "../stores/query-tabs.store";
import { Icon } from "./ui/icons";

function connectionStatusText(status: ConnectionStatus): string {
  if (status === "connecting") return "Connecting";
  if (status === "introspecting") return "Refreshing metadata";
  if (status === "disconnecting") return "Disconnecting";
  if (status === "error") return "Connection failed";
  if (isConnectionReady(status)) return "Connected";
  return "Disconnected";
}

export function StatusBar() {
  const activeTab = useQueryTabsStore((state) =>
    state.tabs.find((tab) => tab.id === state.activeTabId),
  );
  const activeConnection = useConnectionsStore((state) =>
    activeTab?.connectionId
      ? state.connections[activeTab.connectionId]
      : undefined,
  );
  const activeStatus = activeConnection?.status;
  const busy =
    activeStatus === "connecting" ||
    activeStatus === "introspecting" ||
    activeStatus === "disconnecting";
  const statusText = activeStatus
    ? connectionStatusText(activeStatus)
    : "Ready";
  const activeContext = activeTab?.executionContext;
  const contextLabel = activeContext
    ? formatExecutionContext(
        activeContext,
        getExecutionContextCapabilities(dialectForConnection(activeConnection)),
      )
    : null;
  const statusTone = busy
    ? "text-primary"
    : activeStatus === "error"
      ? "text-error"
      : isConnectionReady(activeStatus ?? "disconnected")
        ? "text-success"
        : "text-on-surface-variant";

  return (
    <footer
      className="relative z-20 isolate flex min-w-0 items-center gap-3 border-t border-structural-divider bg-app-statusbar px-3 font-mono text-[10px] text-on-surface-variant"
      data-slot="status-bar"
    >
      <span
        aria-live="polite"
        className={`flex items-center gap-1.5 ${statusTone}`}
        role="status"
      >
        {busy ? (
          <Icon className="animate-spin" name="spinner" size={11} />
        ) : (
          <span className="h-1.5 w-1.5 rounded-full bg-current" />
        )}
        {busy ? `${statusText}…` : statusText}
      </span>
      <span className="hidden sm:inline">
        {activeConnection?.name ?? "No connection"}
      </span>
      {activeConnection ? (
        <span className="hidden md:inline">{activeConnection.engine}</span>
      ) : null}
      {contextLabel && contextLabel !== "No context selected" ? (
        <span className="hidden lg:inline truncate max-w-64">
          {contextLabel}
        </span>
      ) : null}
      <span className="hidden md:inline">TLS 1.3</span>
      <span className="hidden lg:inline">UTF-8</span>
      <span className="ml-auto">
        {APP_NAME} {APP_VERSION_LABEL}
      </span>
    </footer>
  );
}
