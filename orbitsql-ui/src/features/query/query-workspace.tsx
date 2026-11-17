import {
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { COMMAND_IDS } from "../../commands/command-model";
import { registerCommandHandlers } from "../../commands/command-registry";
import { Button } from "../../components/ui/ui";
import { useCatalogStore } from "../../stores/catalog.store";
import {
  isConnectionReady,
  useConnectionsStore,
} from "../../stores/connections.store";
import { openQueryTab, useQueryTabsStore } from "../../stores/query-tabs.store";
import { childrenFromSnapshot } from "../connections/catalog-explorer";
import {
  cancelQuery,
  cancelQuerySession,
  closeQuerySession,
  connectConnectionOnly,
  connectionErrorMessage,
  executeQueryInContext,
  fetchQuerySession,
  pingConnection,
  refreshConnectionCatalog,
  startQuerySessionInContext,
} from "../connections/connection-ipc";
import { contextFromCatalog } from "../connections/execution-context";
import { connectionProfileName } from "../connections/profile-style";
import { ResultGrid } from "./result-grid";
import { MAX_RESULT_ROWS, RESULT_FETCH_SIZE } from "./result-session";

const connectionRecoveryPromises = new Map<string, Promise<void>>();
const runningQueryTabs = new Set<string>();

async function waitForConnectionOperation(connectionId: string) {
  const isBusy = (status: string | undefined) =>
    status === "connecting" ||
    status === "introspecting" ||
    status === "disconnecting";
  if (
    !isBusy(useConnectionsStore.getState().connections[connectionId]?.status)
  ) {
    return;
  }
  await new Promise<void>((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      unsubscribe();
      reject(new Error("Timed out waiting for the connection operation."));
    }, 120_000);
    const unsubscribe = useConnectionsStore.subscribe((state) => {
      if (isBusy(state.connections[connectionId]?.status)) return;
      window.clearTimeout(timeout);
      unsubscribe();
      resolve();
    });
  });
}

function ensureQueryConnection(connectionId: string): Promise<void> {
  const pending = connectionRecoveryPromises.get(connectionId);
  if (pending) return pending;
  const operation = (async () => {
    await waitForConnectionOperation(connectionId);
    const active = await pingConnection(connectionId);
    const store = useConnectionsStore.getState();
    if (active) {
      if (
        !isConnectionReady(
          store.connections[connectionId]?.status ?? "disconnected",
        )
      ) {
        store.setConnectionStatus(connectionId, "ready");
      }
      return;
    }
    store.setConnectionStatus(connectionId, "connecting");
    await connectConnectionOnly(connectionId);
    store.setConnectionStatus(connectionId, "introspecting");
    const snapshot = await refreshConnectionCatalog(connectionId);
    useCatalogStore.getState().setSnapshot(snapshot);
    store.setConnectionChildren(connectionId, childrenFromSnapshot(snapshot));
    store.setConnectionRefreshed(connectionId);
    store.setConnectionStatus(connectionId, "ready");
  })();
  connectionRecoveryPromises.set(connectionId, operation);
  const clearOperation = () => {
    if (connectionRecoveryPromises.get(connectionId) === operation) {
      connectionRecoveryPromises.delete(connectionId);
    }
  };
  void operation.then(clearOperation, clearOperation);
  return operation;
}

function sameExecutionContext(
  left: import("../connections/execution-context").ExecutionContext,
  right: import("../connections/execution-context").ExecutionContext,
) {
  return (
    left.connectionId === right.connectionId &&
    left.connectionState === right.connectionState &&
    left.dimensions.database === right.dimensions.database &&
    left.dimensions.schema === right.dimensions.schema &&
    left.dimensions.catalog === right.dimensions.catalog &&
    left.dimensions.service === right.dimensions.service &&
    left.dimensions.container === right.dimensions.container &&
    left.dimensions.user === right.dimensions.user
  );
}

function sqlForExecution(
  sql: string,
  selection: { from: number; to: number } | null,
) {
  if (selection && selection.from !== selection.to) {
    return sql.slice(selection.from, selection.to).trim();
  }
  if (!sql.includes(";")) return sql.trim();
  const cursor = Math.min(Math.max(selection?.from ?? 0, 0), sql.length);
  const previousDelimiter = sql.lastIndexOf(";", Math.max(0, cursor - 1));
  const start = previousDelimiter + 1;
  const endIndex = sql.indexOf(";", cursor);
  const end = endIndex === -1 ? sql.length : endIndex;
  const currentStatement = sql.slice(start, end).trim();

  if (currentStatement) return currentStatement;
  if (previousDelimiter === -1) return sql.slice(0, endIndex).trim();

  const priorEnd = previousDelimiter;
  const priorStart = sql.lastIndexOf(";", Math.max(0, priorEnd - 1)) + 1;
  return sql.slice(priorStart, priorEnd).trim();
}

import { SqlEditor } from "../editor/codemirror/sql-editor";
import { createEmptyCatalogIndex } from "../sql-intelligence/catalog";

export function QueryWorkspaceScreen({
  embedded = false,
}: {
  embedded?: boolean;
}) {
  const tabs = useQueryTabsStore((state) => state.tabs);
  const activeTabId = useQueryTabsStore((state) => state.activeTabId);
  const activeTab = tabs.find((tab) => tab.id === activeTabId);
  const updateTab = useQueryTabsStore((state) => state.updateTab);
  const setExecutionContext = useQueryTabsStore(
    (state) => state.setExecutionContext,
  );
  const connections = useConnectionsStore((state) => state.connections);
  const catalogs = useCatalogStore((state) => state.snapshots);
  const setActiveConnection = useConnectionsStore(
    (state) => state.setActiveConnection,
  );
  const editorSelectionRef = useRef<{
    tabId: string | null;
    from: number;
    to: number;
  }>({ tabId: null, from: 0, to: 0 });
  const [resultsHeightPercent, setResultsHeightPercent] = useState<
    number | null
  >(null);
  const [isResizingResults, setIsResizingResults] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [batchSize, setBatchSize] = useState(RESULT_FETCH_SIZE);
  const loadingMoreRef = useRef(false);
  const resultsResizeRef = useRef<{
    pointerId: number;
    startY: number;
    startHeight: number;
    containerHeight: number;
  } | null>(null);
  const splitContainerRef = useRef<HTMLDivElement>(null);
  const selectedConnection = activeTab?.connectionId
    ? connections[activeTab.connectionId]
    : undefined;
  const editorCatalog = useMemo(() => {
    if (activeTab?.connectionId) {
      const connectionCatalog = catalogs.get(activeTab.connectionId);
      if (connectionCatalog) return connectionCatalog;
    }
    const dialect =
      selectedConnection?.engine === "PostgreSQL"
        ? "postgresql"
        : selectedConnection?.engine === "MySQL"
          ? "mysql"
          : selectedConnection?.engine === "MariaDB"
            ? "mariadb"
            : selectedConnection?.engine === "Microsoft SQL Server"
              ? "sqlserver"
              : "generic";
    return createEmptyCatalogIndex(dialect);
  }, [activeTab?.connectionId, catalogs, selectedConnection?.engine]);
  const tabCatalog = activeTab?.connectionId
    ? catalogs.get(activeTab.connectionId)
    : undefined;
  const executionContext = useMemo(
    () =>
      contextFromCatalog(
        selectedConnection,
        tabCatalog,
        activeTab?.executionContext,
      ),
    [activeTab?.executionContext, selectedConnection, tabCatalog],
  );
  const canRun = Boolean(activeTab && selectedConnection);
  const connectionReady = Boolean(
    selectedConnection && isConnectionReady(selectedConnection.status),
  );

  useEffect(() => {
    if (embedded) return;
    setActiveConnection(activeTab?.connectionId ?? null);
  }, [activeTab?.connectionId, embedded, setActiveConnection]);

  useEffect(() => {
    if (
      activeTab &&
      !sameExecutionContext(activeTab.executionContext, executionContext)
    ) {
      setExecutionContext(activeTab.id, executionContext);
    }
  }, [activeTab, executionContext, setExecutionContext]);

  const runQuery = useCallback(async () => {
    const queryTabs = useQueryTabsStore.getState();
    const tabId = queryTabs.activeTabId;
    const queryTab = queryTabs.tabs.find((tab) => tab.id === tabId);
    if (!tabId || !queryTab) return;
    if (runningQueryTabs.has(tabId)) return;
    const connectionId = queryTab.connectionId;
    const originalQuery = queryTab.query;
    const currentSelection = editorSelectionRef.current;
    const sql =
      currentSelection.tabId === tabId
        ? sqlForExecution(originalQuery, currentSelection)
        : originalQuery.trim();
    const connection = connectionId
      ? useConnectionsStore.getState().connections[connectionId]
      : undefined;

    if (!connectionId || !connection) {
      updateTab(tabId, {
        errorMessage: "Select a database connection before running a query.",
        running: false,
      });
      return;
    }
    if (!sql) {
      updateTab(tabId, {
        errorMessage: "Enter a SQL query before running it.",
        running: false,
      });
      return;
    }

    runningQueryTabs.add(tabId);
    const previousSessionId = queryTab.result?.sessionId;
    updateTab(tabId, { errorMessage: null, result: null, running: true });
    setLoadingMore(false);
    let connectionReadyForExecution = false;
    try {
      if (previousSessionId) {
        try {
          await closeQuerySession(previousSessionId);
        } catch {}
      }
      await ensureQueryConnection(connectionId);
      connectionReadyForExecution = true;
      const currentConnection =
        useConnectionsStore.getState().connections[connectionId] ?? connection;
      const currentCatalog = useCatalogStore
        .getState()
        .snapshots.get(connectionId);
      const currentExecutionContext = contextFromCatalog(
        currentConnection,
        currentCatalog,
        queryTab.executionContext,
      );
      const session = await startQuerySessionInContext(
        connectionId,
        sql,
        {
          database: currentExecutionContext.dimensions.database,
          schema: currentExecutionContext.dimensions.schema,
        },
        batchSize,
      );
      const firstBatch = session.firstBatch;
      updateTab(tabId, {
        errorMessage: null,
        lastExecutedQuery: originalQuery,
        result: {
          columns: session.columns,
          rows: firstBatch.rows,
          rowCount: firstBatch.rowCount,
          affectedRows: null,
          elapsedMs: session.elapsedMs,
          sessionId: firstBatch.hasMore ? session.sessionId : null,
          hasMore: firstBatch.hasMore,
          totalFetched: firstBatch.totalFetched,
          rowLimitReached: false,
        },
        running: false,
      });
      setLoadingMore(false);
    } catch (error) {
      const message = connectionErrorMessage(error);
      if (connectionReadyForExecution) {
        try {
          const active = await pingConnection(connectionId);
          useConnectionsStore
            .getState()
            .setConnectionStatus(
              connectionId,
              active ? "ready" : "disconnected",
            );
        } catch {
          useConnectionsStore
            .getState()
            .setConnectionStatus(connectionId, "error");
        }
      } else {
        useConnectionsStore
          .getState()
          .setConnectionStatus(connectionId, "error");
      }
      updateTab(tabId, {
        errorMessage: connectionReadyForExecution
          ? message
          : `Connection failed; query was not executed. ${message}`,
        result: null,
        running: false,
      });
      setLoadingMore(false);
    } finally {
      runningQueryTabs.delete(tabId);
    }
  }, [batchSize, updateTab]);

  const fetchMoreResults = useCallback(async () => {
    const tabId = useQueryTabsStore.getState().activeTabId;
    const currentTab = useQueryTabsStore
      .getState()
      .tabs.find((tab) => tab.id === tabId);
    const currentResult = currentTab?.result;
    const sessionId = currentResult?.sessionId;
    if (
      !tabId ||
      !currentTab ||
      !sessionId ||
      currentTab?.running ||
      !currentResult?.hasMore ||
      currentResult.totalFetched >= MAX_RESULT_ROWS ||
      loadingMore ||
      loadingMoreRef.current
    ) {
      return;
    }
    loadingMoreRef.current = true;
    setLoadingMore(true);
    try {
      const requestedRows = Math.min(
        batchSize,
        MAX_RESULT_ROWS - currentResult.totalFetched,
      );
      const batch = await fetchQuerySession(sessionId, requestedRows);
      const latestTab = useQueryTabsStore
        .getState()
        .tabs.find((tab) => tab.id === tabId);
      if (latestTab?.result?.sessionId !== sessionId) return;
      const reachedLimit =
        batch.totalFetched >= MAX_RESULT_ROWS && batch.hasMore;
      if (reachedLimit) {
        await closeQuerySession(sessionId);
      }
      updateTab(tabId, {
        errorMessage: null,
        result: {
          ...latestTab.result,
          rows: [...latestTab.result.rows, ...batch.rows],
          rowCount: latestTab.result.rowCount + batch.rowCount,
          sessionId: batch.hasMore && !reachedLimit ? sessionId : null,
          hasMore: batch.hasMore && !reachedLimit,
          totalFetched: batch.totalFetched,
          rowLimitReached: reachedLimit,
        },
      });
    } catch (error) {
      const latestTab = useQueryTabsStore
        .getState()
        .tabs.find((tab) => tab.id === tabId);
      if (latestTab?.running || latestTab?.result?.sessionId !== sessionId) {
        return;
      }
      updateTab(tabId, { errorMessage: connectionErrorMessage(error) });
    } finally {
      loadingMoreRef.current = false;
      setLoadingMore(false);
    }
  }, [batchSize, loadingMore, updateTab]);

  useEffect(() => {
    return registerCommandHandlers(
      { [COMMAND_IDS.queryRun]: runQuery },
      {
        [COMMAND_IDS.queryRun]: {
          enabled: canRun && !activeTab?.running,
        },
        [COMMAND_IDS.queryStop]: { enabled: false },
      },
    );
  }, [activeTab?.running, canRun, runQuery]);

  function updateQuery(query: string) {
    if (!activeTab) return;
    updateTab(activeTab.id, { errorMessage: null, query });
  }

  async function runTransactionCommand(command: "COMMIT" | "ROLLBACK") {
    if (!activeTab || !selectedConnection || !connectionReady) return;
    updateTab(activeTab.id, { errorMessage: null, running: true });
    try {
      await executeQueryInContext(selectedConnection.id, command, {
        database: executionContext.dimensions.database,
        schema: executionContext.dimensions.schema,
      });
      updateTab(activeTab.id, { errorMessage: null, running: false });
    } catch (error) {
      updateTab(activeTab.id, {
        errorMessage: connectionErrorMessage(error),
        running: false,
      });
    }
  }

  function createTab() {
    openQueryTab(useConnectionsStore.getState().activeConnectionId);
  }

  const stopResultsResize = useCallback(() => {
    resultsResizeRef.current = null;
    setIsResizingResults(false);
  }, []);

  const resizeResults = useCallback((event: globalThis.PointerEvent) => {
    const current = resultsResizeRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const minimumEditorHeight = 250;
    const minimumResultsHeight = 220;
    const maximumResultsHeight =
      current.containerHeight - minimumEditorHeight - 1;
    const nextHeight = Math.min(
      maximumResultsHeight,
      Math.max(
        minimumResultsHeight,
        current.startHeight - (event.clientY - current.startY),
      ),
    );
    setResultsHeightPercent((nextHeight / current.containerHeight) * 100);
  }, []);

  const startResultsResize = useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      if (event.button !== 0) return;
      const container = splitContainerRef.current;
      const results = container?.children[2];
      if (!container || !(results instanceof HTMLElement)) return;
      const containerHeight = container.getBoundingClientRect().height;
      resultsResizeRef.current = {
        pointerId: event.pointerId,
        startY: event.clientY,
        startHeight: results.getBoundingClientRect().height,
        containerHeight,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
      setIsResizingResults(true);
    },
    [],
  );

  const resizeResultsWithKeyboard = useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      const container = splitContainerRef.current;
      if (!container) return;
      const containerHeight = container.getBoundingClientRect().height;
      const step = event.shiftKey ? 8 : 4;
      const current = resultsHeightPercent ?? 45;
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setResultsHeightPercent(Math.min(100, current + step));
      }
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setResultsHeightPercent(Math.max(0, current - step));
      }
      if (event.key === "Home") {
        event.preventDefault();
        setResultsHeightPercent((220 / containerHeight) * 100);
      }
      if (event.key === "End") {
        event.preventDefault();
        setResultsHeightPercent(
          ((containerHeight - 250 - 1) / containerHeight) * 100,
        );
      }
    },
    [resultsHeightPercent],
  );

  useEffect(() => {
    if (!isResizingResults) return;
    const handlePointerMove = (event: globalThis.PointerEvent) => {
      resizeResults(event);
    };
    const handlePointerEnd = (event: globalThis.PointerEvent) => {
      if (resultsResizeRef.current?.pointerId !== event.pointerId) return;
      stopResultsResize();
    };
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerEnd);
    window.addEventListener("pointercancel", handlePointerEnd);
    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerEnd);
      window.removeEventListener("pointercancel", handlePointerEnd);
    };
  }, [isResizingResults, resizeResults, stopResultsResize]);

  if (!activeTab) {
    return (
      <div className="flex h-full min-h-0 flex-col items-center justify-center gap-3 bg-app-content px-4 text-center">
        <p className="font-mono text-label-caps uppercase ds-tracking-caps-wide text-primary">
          No query tabs open
        </p>
        <p className="text-body-sm text-on-surface-variant">
          Open a new query to start working.
        </p>
        {!embedded ? (
          <Button icon="plus" onClick={createTab} variant="primary">
            New Query
          </Button>
        ) : null}
      </div>
    );
  }

  const containerClassName = "flex h-full min-h-0 flex-col bg-app-content";

  return (
    <div className={containerClassName}>
      <div
        className="flex h-[34px] min-h-[34px] shrink-0 items-center justify-start gap-2 overflow-x-auto whitespace-nowrap border-b border-structural-divider bg-app-toolbar px-2"
        data-slot="query-command-bar"
      >
        <div className="flex shrink-0 items-center gap-1.5">
          <Button
            className="h-7 shrink-0 rounded-sm px-2"
            disabled={activeTab.running}
            icon="play"
            onClick={() => void runQuery()}
            variant="primary"
          >
            {activeTab.running ? "Executing..." : "Execute"}
          </Button>
          <span
            aria-hidden="true"
            className="mx-1 h-5 shrink-0 border-l border-structural-divider"
          />
          <div className="flex shrink-0 items-center gap-0.5">
            <Button
              className="h-7 rounded-sm px-1.5 text-on-surface-variant hover:bg-success/10 hover:text-success disabled:opacity-60"
              disabled={!connectionReady || activeTab.running}
              icon="check"
              onClick={() => void runTransactionCommand("COMMIT")}
              variant="ghost"
            >
              Commit
            </Button>
            <Button
              className="h-7 rounded-sm px-1.5 text-on-surface-variant hover:bg-error/10 hover:text-error disabled:opacity-60"
              disabled={!connectionReady || activeTab.running}
              icon="rollback"
              onClick={() => void runTransactionCommand("ROLLBACK")}
              variant="ghost"
            >
              Rollback
            </Button>
          </div>
        </div>
      </div>

      <div
        className={`grid min-h-0 flex-1 grid-rows-[minmax(250px,1fr)_1px_minmax(220px,0.82fr)] ${isResizingResults ? "select-none" : ""}`}
        ref={splitContainerRef}
        style={
          resultsHeightPercent === null
            ? undefined
            : {
                gridTemplateRows: `minmax(250px, 1fr) 1px minmax(220px, ${resultsHeightPercent}%)`,
              }
        }
      >
        <section className="flex min-h-0 flex-col overflow-hidden bg-app-content">
          <SqlEditor
            catalog={editorCatalog}
            className="h-full min-h-0 flex-1 overflow-hidden border-b border-structural-divider bg-app-content"
            documentId={activeTab.id}
            executionContext={executionContext.dimensions}
            onChange={updateQuery}
            onSelectionChange={(selection) => {
              const nextSelection = { tabId: activeTab.id, ...selection };
              editorSelectionRef.current = nextSelection;
            }}
            value={activeTab.query}
          />
        </section>

        <button
          aria-label="Resize query results panel"
          aria-orientation="horizontal"
          aria-valuemax={100}
          aria-valuemin={0}
          aria-valuenow={resultsHeightPercent ?? 45}
          className={`query-results-resize-handle ${isResizingResults ? "is-resizing" : ""}`}
          onKeyDown={resizeResultsWithKeyboard}
          onPointerCancel={stopResultsResize}
          onPointerDown={startResultsResize}
          onPointerUp={stopResultsResize}
          role="slider"
          tabIndex={0}
          type="button"
        />

        <ResultGrid
          errorMessage={activeTab.errorMessage}
          onRefresh={() => void runQuery()}
          onResultTabChange={(resultTab) =>
            updateTab(activeTab.id, { resultTab })
          }
          result={activeTab.result}
          resultTab={activeTab.resultTab}
          running={activeTab.running}
          profile={connectionProfileName(selectedConnection?.detail)}
          profileColor={selectedConnection?.profileColor}
          onCancel={() => {
            if (!selectedConnection) return;
            const sessionId = activeTab.result?.sessionId;
            const cancel = sessionId
              ? cancelQuerySession(sessionId)
              : cancelQuery(selectedConnection.id).then(() => undefined);
            void cancel.then(() => {
              updateTab(activeTab.id, {
                errorMessage: "Query cancelled",
                running: false,
                result: activeTab.result
                  ? {
                      ...activeTab.result,
                      sessionId: null,
                      hasMore: false,
                    }
                  : null,
              });
            });
          }}
          loadingMore={loadingMore}
          batchSize={batchSize}
          onBatchSizeChange={setBatchSize}
          onLoadMore={() => void fetchMoreResults()}
        />
      </div>
    </div>
  );
}
