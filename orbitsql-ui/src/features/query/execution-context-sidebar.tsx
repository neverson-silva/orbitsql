import { useMemo } from "react";
import { useCatalogStore } from "../../stores/catalog.store";
import { useConnectionsStore } from "../../stores/connections.store";
import { useQueryTabsStore } from "../../stores/query-tabs.store";
import {
  connectionErrorMessage,
  executeQuery,
} from "../connections/connection-ipc";
import {
  contextFromCatalog,
  contextSwitchSql,
  dialectForConnection,
  type ExecutionContext,
} from "../connections/execution-context";
import { ExecutionContextPicker } from "./execution-context-selector";

function noExecutionContext(): ExecutionContext {
  return {
    connectionId: null,
    dimensions: {},
    connectionState: "disconnected",
  };
}

export function ExecutionContextSidebar() {
  const tabs = useQueryTabsStore((state) => state.tabs);
  const activeTabId = useQueryTabsStore((state) => state.activeTabId);
  const activeTab = tabs.find((tab) => tab.id === activeTabId);
  const updateTab = useQueryTabsStore((state) => state.updateTab);
  const setExecutionContext = useQueryTabsStore(
    (state) => state.setExecutionContext,
  );
  const connections = useConnectionsStore((state) => state.connections);
  const setActiveConnection = useConnectionsStore(
    (state) => state.setActiveConnection,
  );
  const catalogs = useCatalogStore((state) => state.snapshots);
  const availableConnections = useMemo(
    () => Object.values(connections),
    [connections],
  );
  const connection = activeTab?.connectionId
    ? connections[activeTab.connectionId]
    : undefined;
  const catalog = activeTab?.connectionId
    ? catalogs.get(activeTab.connectionId)
    : undefined;
  const context = activeTab
    ? contextFromCatalog(connection, catalog, activeTab.executionContext)
    : noExecutionContext();

  function updateConnection(connectionId: string) {
    if (!activeTab) return;
    const nextConnection = connections[connectionId];
    const nextContext: ExecutionContext = {
      connectionId: connectionId || null,
      dimensions: {},
      connectionState: nextConnection?.status ?? "disconnected",
    };
    updateTab(activeTab.id, {
      connectionId: connectionId || null,
      errorMessage: null,
      result: null,
      lastExecutedQuery: null,
    });
    setExecutionContext(activeTab.id, nextContext);
    setActiveConnection(connectionId || null);
  }

  async function updateDimension(
    dimension: Parameters<typeof contextSwitchSql>[1],
    value: string,
  ) {
    if (!activeTab || !connection) return;
    if (!value) {
      const dimensions = { ...context.dimensions };
      delete dimensions[dimension];
      setExecutionContext(activeTab.id, { ...context, dimensions });
      updateTab(activeTab.id, { errorMessage: null });
      return;
    }
    const sql = contextSwitchSql(
      dialectForConnection(connection),
      dimension,
      value,
    );
    if (!sql) {
      setExecutionContext(activeTab.id, {
        ...context,
        dimensions: { ...context.dimensions, [dimension]: value },
      });
      updateTab(activeTab.id, { errorMessage: null });
      return;
    }
    try {
      await executeQuery(connection.id, sql);
      setExecutionContext(activeTab.id, {
        ...context,
        dimensions: { ...context.dimensions, [dimension]: value },
      });
      updateTab(activeTab.id, { errorMessage: null });
    } catch (error) {
      updateTab(activeTab.id, {
        errorMessage: connectionErrorMessage(error),
      });
    }
  }

  return (
    <ExecutionContextPicker
      catalog={catalog}
      connection={connection}
      connections={availableConnections}
      context={context}
      onConnectionSelect={updateConnection}
      onSelect={(dimension, value) => void updateDimension(dimension, value)}
    />
  );
}
