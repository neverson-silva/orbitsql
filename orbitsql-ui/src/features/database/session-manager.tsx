import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
} from "../../components/ui/drawer";
import { Icon } from "../../components/ui/icons";
import {
  Button,
  ScreenHeader,
  StatusPill,
  Surface,
} from "../../components/ui/ui";
import {
  isConnectionReady,
  useConnectionsStore,
} from "../../stores/connections.store";
import {
  connectionErrorMessage,
  type DatabaseSession,
  getDatabaseSessions,
  killDatabaseSession,
  type SessionTelemetry,
} from "../connections/connection-ipc";
import {
  createSessionRequestGate,
  filterAndSortSessions,
  isSessionPermissionError,
  LONG_RUNNING_THRESHOLD_MS,
  type SessionFilter,
  type SessionSortKey,
} from "./session-manager-model";
import { SessionSqlViewer } from "./session-sql-viewer";

const REFRESH_INTERVALS = [1, 2, 5, 10, 30] as const;
const sessionRequestGate = createSessionRequestGate();

export function SessionManagerScreen() {
  const activeConnectionId = useConnectionsStore(
    (state) => state.activeConnectionId,
  );
  const connection = useConnectionsStore((state) =>
    activeConnectionId ? state.connections[activeConnectionId] : undefined,
  );
  const connected = Boolean(connection && isConnectionReady(connection.status));
  const isSqlServer = connection?.engine === "Microsoft SQL Server";
  const [telemetry, setTelemetry] = useState<SessionTelemetry | null>(null);
  const [telemetryConnectionId, setTelemetryConnectionId] = useState<
    string | null
  >(null);
  const [loading, setLoading] = useState(false);
  const [refreshFailed, setRefreshFailed] = useState<string | null>(null);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [intervalSeconds, setIntervalSeconds] = useState<number>(2);
  const [filter, setFilter] = useState<SessionFilter>("all");
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<{
    key: SessionSortKey;
    descending: boolean;
  }>({
    key: "priority",
    descending: false,
  });
  const [selectedSessionId, setSelectedSessionId] = useState<number | null>(
    null,
  );
  const [killTarget, setKillTarget] = useState<DatabaseSession | null>(null);
  const [killing, setKilling] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const generation = useRef(0);
  const lastUpdatedAt = useRef<number | null>(null);

  const refresh = useCallback(async (connectionId: string) => {
    const requestGeneration = generation.current;
    setLoading(true);
    try {
      const result = await sessionRequestGate.run(connectionId, () =>
        getDatabaseSessions(connectionId),
      );
      if (generation.current !== requestGeneration) return;
      setTelemetry(result);
      setTelemetryConnectionId(connectionId);
      setRefreshFailed(null);
      lastUpdatedAt.current = Date.now();
    } catch (error) {
      if (generation.current === requestGeneration) {
        setRefreshFailed(connectionErrorMessage(error));
      }
    } finally {
      if (generation.current === requestGeneration) setLoading(false);
    }
  }, []);

  useEffect(() => {
    generation.current += 1;
    const requestGeneration = generation.current;
    setTelemetry(null);
    setTelemetryConnectionId(null);
    setRefreshFailed(null);
    setSelectedSessionId(null);
    lastUpdatedAt.current = null;
    if (!connected || !isSqlServer || !activeConnectionId) {
      setLoading(false);
      return;
    }
    void refresh(activeConnectionId);
    return () => {
      if (generation.current === requestGeneration) generation.current += 1;
    };
  }, [activeConnectionId, connected, isSqlServer, refresh]);

  useEffect(() => {
    if (!autoRefresh || !connected || !isSqlServer || !activeConnectionId)
      return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let disposed = false;
    const schedule = () => {
      timer = setTimeout(async () => {
        if (disposed) return;
        await refresh(activeConnectionId);
        if (!disposed) schedule();
      }, intervalSeconds * 1000);
    };
    schedule();
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
    };
  }, [
    activeConnectionId,
    autoRefresh,
    connected,
    intervalSeconds,
    isSqlServer,
    refresh,
  ]);

  const sessions =
    telemetryConnectionId === activeConnectionId
      ? (telemetry?.sessions ?? [])
      : [];
  const filteredSessions = filterAndSortSessions(
    sessions,
    filter,
    search,
    sort,
  );
  const selectedSession =
    sessions.find((session) => session.sessionId === selectedSessionId) ?? null;
  const activeCount = sessions.filter(
    (session) => session.requestStatus,
  ).length;
  const blockedCount = sessions.filter(
    (session) => session.blockingSessionId && session.blockingSessionId !== 0,
  ).length;
  const longCount = sessions.filter(
    (session) =>
      BigInt(session.durationMs) >= BigInt(LONG_RUNNING_THRESHOLD_MS),
  ).length;
  const permissionError = isSessionPermissionError(refreshFailed);

  const manualRefresh = () => {
    if (activeConnectionId && connected && isSqlServer)
      void refresh(activeConnectionId);
  };

  const confirmKill = async () => {
    if (!activeConnectionId || !killTarget || killing) return;
    setKilling(true);
    setActionError(null);
    try {
      await killDatabaseSession(activeConnectionId, killTarget.sessionId);
      setKillTarget(null);
      await refresh(activeConnectionId);
    } catch (error) {
      setActionError(connectionErrorMessage(error));
    } finally {
      setKilling(false);
    }
  };

  const requestSort = (key: SessionSortKey) => {
    setSort((current) =>
      current.key === key
        ? { key, descending: !current.descending }
        : { key, descending: key !== "sessionId" && key !== "status" },
    );
  };

  const lastUpdated =
    lastUpdatedAt.current === null
      ? "Not updated"
      : new Date(lastUpdatedAt.current).toLocaleTimeString();

  return (
    <div className="mx-auto flex h-full max-w-[1700px] flex-col gap-4 overflow-hidden p-5 lg:p-7">
      <ScreenHeader
        eyebrow="Live connection telemetry"
        title="Database Session &amp; Process Manager"
        description="Observe active database sessions, wait states, lock trees, and long-running queries from the connected database."
      />
      {!connected ? (
        <UnavailableState
          title="Session telemetry is not available yet"
          description="Connect to a SQL Server instance to inspect sessions and processes."
        />
      ) : !isSqlServer ? (
        <UnavailableState
          title="Session telemetry is not supported for this provider"
          description="Session and process telemetry is currently available for Microsoft SQL Server connections."
        />
      ) : permissionError && sessions.length === 0 ? (
        <UnavailableState
          title="Additional SQL Server permissions are required"
          description="The current login cannot view server session telemetry. SQL Server 2019 and earlier commonly require VIEW SERVER STATE; SQL Server 2022 and later require VIEW SERVER PERFORMANCE STATE."
          action={
            <Button variant="secondary" onClick={manualRefresh}>
              Retry
            </Button>
          }
        />
      ) : refreshFailed && sessions.length === 0 ? (
        <UnavailableState
          title="Unable to load session telemetry"
          description={refreshFailed}
          action={
            <Button variant="secondary" onClick={manualRefresh}>
              Retry
            </Button>
          }
        />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <div className="flex flex-wrap items-center gap-1 rounded-md border border-outline-variant/70 bg-surface-raised p-1">
              {filterOption("all", `All ${sessions.length}`)}
              {filterOption("active", `Active ${activeCount}`)}
              {filterOption("blocked", `Blocked ${blockedCount}`)}
              {filterOption(
                "blocking",
                `Blocking ${sessions.filter((session) => session.isBlocking).length}`,
              )}
              {filterOption("long", `Long ${longCount}`)}
              {filterOption("idle", `Idle ${sessions.length - activeCount}`)}
            </div>
            <label className="flex min-w-48 flex-1 items-center gap-2 rounded-md border border-outline-variant bg-surface-raised px-3 py-2 text-on-surface-variant">
              <Icon name="search" size={14} />
              <input
                aria-label="Search sessions"
                className="min-w-0 flex-1 bg-transparent text-body-sm text-on-surface outline-none placeholder:text-on-surface-variant"
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Search sessions, SQL text..."
                value={search}
              />
            </label>
            <label className="flex items-center gap-2 text-label-sm text-on-surface-variant">
              <input
                checked={autoRefresh}
                onChange={(event) => setAutoRefresh(event.target.checked)}
                type="checkbox"
              />
              Auto refresh
            </label>
            <select
              aria-label="Refresh interval"
              className="h-9 rounded-md border border-outline-variant bg-surface-raised px-2 text-body-sm"
              disabled={!autoRefresh}
              onChange={(event) =>
                setIntervalSeconds(Number(event.target.value))
              }
              value={intervalSeconds}
            >
              {REFRESH_INTERVALS.map((value) => (
                <option key={value} value={value}>
                  {value}s
                </option>
              ))}
            </select>
            <span className="text-label-sm text-on-surface-variant">
              Updated {lastUpdated}
            </span>
            <Button
              aria-label="Refresh sessions"
              disabled={loading}
              icon="refresh"
              onClick={manualRefresh}
              variant="secondary"
            >
              Refresh
            </Button>
          </div>
          {refreshFailed ? (
            <div className="rounded-md border border-warning/30 bg-warning-container/30 px-3 py-2 text-body-sm text-on-surface">
              {permissionError
                ? "Refresh failed: the current login no longer has permission to view session telemetry."
                : `Refresh failed: ${refreshFailed}`}{" "}
              <span className="text-on-surface-variant">
                · Last successful update {lastUpdated}
              </span>
            </div>
          ) : null}
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            <Summary label="Sessions" value={sessions.length} />
            <Summary label="Active" value={activeCount} />
            <Summary
              label="Blocked"
              value={blockedCount}
              tone={blockedCount ? "error" : "neutral"}
            />
            <Summary
              label="Long running"
              value={longCount}
              tone={longCount ? "warning" : "neutral"}
            />
          </div>
          <Surface className="min-h-0 flex-1 overflow-hidden">
            <div className="h-full overflow-auto">
              <table className="w-full min-w-[1180px] border-collapse text-left text-body-sm">
                <thead className="sticky top-0 z-10 bg-surface-subtle text-label-caps text-on-surface-variant">
                  <tr>
                    {[
                      ["sessionId", "SPID"],
                      ["status", "Status"],
                      ["durationMs", "Duration"],
                      ["database", "Database"],
                      ["login", "Login"],
                      ["host", "Host / application"],
                      ["command", "Command"],
                      ["cpuTimeMs", "CPU"],
                      ["reads", "Reads"],
                      ["writes", "Writes"],
                      ["waitType", "Wait"],
                      ["blockingSessionId", "Blocking"],
                      ["query", "Query"],
                    ].map(([key, label]) => (
                      <th
                        className="whitespace-nowrap border-b border-outline-variant px-3 py-2 font-medium"
                        key={key}
                      >
                        <button
                          type="button"
                          className="hover:text-on-surface"
                          onClick={() => requestSort(key as SessionSortKey)}
                        >
                          {label}
                          {sort.key === key
                            ? sort.descending
                              ? " ↓"
                              : " ↑"
                            : ""}
                        </button>
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {filteredSessions.map((session) => (
                    <tr
                      className={`cursor-pointer border-b border-outline-variant/50 hover:bg-surface-subtle ${selectedSessionId === session.sessionId ? "bg-selected-surface" : ""}`}
                      key={session.sessionId}
                      onClick={() => setSelectedSessionId(session.sessionId)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter")
                          setSelectedSessionId(session.sessionId);
                      }}
                      tabIndex={0}
                    >
                      <td className="px-3 py-2 font-mono">
                        {session.sessionId}
                        {session.isRootBlocker ? (
                          <span
                            className="ml-1 text-warning"
                            title="Root blocker"
                          >
                            ◆
                          </span>
                        ) : null}
                      </td>
                      <td className="px-3 py-2">
                        <SessionStatus session={session} />
                      </td>
                      <td className="whitespace-nowrap px-3 py-2 font-mono">
                        {formatDuration(session.durationMs)}
                        {session.percentComplete > 0 ? (
                          <div className="mt-1 h-1 w-24 overflow-hidden rounded bg-surface-subtle">
                            <div
                              className="h-full bg-primary"
                              style={{
                                width: `${Math.min(100, session.percentComplete)}%`,
                              }}
                            />
                          </div>
                        ) : null}
                      </td>
                      <td className="px-3 py-2">{session.database ?? "—"}</td>
                      <td className="px-3 py-2">{session.login ?? "—"}</td>
                      <td
                        className="max-w-48 truncate px-3 py-2"
                        title={`${session.host ?? ""} / ${session.application ?? ""}`}
                      >
                        {session.host ?? "—"}
                        <span className="block truncate text-label-sm text-on-surface-variant">
                          {session.application ?? ""}
                        </span>
                      </td>
                      <td className="px-3 py-2">{session.command ?? "—"}</td>
                      <td className="px-3 py-2 font-mono">
                        {formatCount(session.cpuTimeMs)}ms
                      </td>
                      <td className="px-3 py-2 font-mono">
                        {formatCount(session.reads)}
                      </td>
                      <td className="px-3 py-2 font-mono">
                        {formatCount(session.writes)}
                      </td>
                      <td
                        className="max-w-36 truncate px-3 py-2 font-mono text-label-sm"
                        title={session.waitType ?? ""}
                      >
                        {session.waitType ?? session.lastWaitType ?? "—"}
                      </td>
                      <td className="px-3 py-2">
                        {session.blockingSessionId &&
                        session.blockingSessionId > 0 ? (
                          <button
                            type="button"
                            className="font-mono text-error underline"
                            onClick={(event) => {
                              event.stopPropagation();
                              setSelectedSessionId(session.blockingSessionId);
                            }}
                          >
                            {session.blockingSessionId}
                          </button>
                        ) : session.blockingSessionId &&
                          session.blockingSessionId < 0 ? (
                          `Resource ${session.blockingSessionId}`
                        ) : (
                          "—"
                        )}
                      </td>
                      <td className="max-w-72 px-3 py-2">
                        <button
                          type="button"
                          className="block max-w-72 truncate text-left font-mono text-label-sm text-on-surface-variant hover:text-primary"
                          onClick={(event) => {
                            event.stopPropagation();
                            setSelectedSessionId(session.sessionId);
                          }}
                          title={
                            session.currentStatement ?? session.batchText ?? ""
                          }
                        >
                          {session.currentStatement ?? session.batchText ?? "—"}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {!loading && filteredSessions.length === 0 ? (
                <div className="flex min-h-40 items-center justify-center p-6 text-center text-body-sm text-on-surface-variant">
                  {sessions.length === 0
                    ? "No user sessions are visible to this login."
                    : "No sessions match the current filters."}
                </div>
              ) : null}
              {loading && sessions.length === 0 ? (
                <div className="p-6 text-body-sm text-on-surface-variant">
                  Loading live SQL Server sessions…
                </div>
              ) : null}
            </div>
          </Surface>
        </div>
      )}
      <SessionDetails
        session={selectedSession}
        open={selectedSession !== null}
        onClose={() => setSelectedSessionId(null)}
        onKill={() => selectedSession && setKillTarget(selectedSession)}
        onSelectSession={setSelectedSessionId}
        monitorSessionId={telemetry?.monitorSessionId ?? null}
      />
      <Dialog
        onOpenChange={(open) => {
          if (!open && !killing) {
            setKillTarget(null);
            setActionError(null);
          }
        }}
        open={killTarget !== null}
      >
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>Kill session #{killTarget?.sessionId}?</DialogTitle>
            <DialogDescription>
              This terminates the SQL Server session. An active transaction may
              roll back.
            </DialogDescription>
          </DialogHeader>
          {killTarget ? (
            <div className="grid grid-cols-2 gap-x-4 gap-y-2 rounded-md bg-surface-subtle p-3 text-body-sm">
              <ContextValue label="Database" value={killTarget.database} />
              <ContextValue label="Login" value={killTarget.login} />
              <ContextValue label="Host" value={killTarget.host} />
              <ContextValue
                label="Application"
                value={killTarget.application}
              />
              <ContextValue
                label="Duration"
                value={formatDuration(killTarget.durationMs)}
              />
              <ContextValue
                label="Statement"
                value={killTarget.currentStatement}
                className="col-span-2 font-mono"
              />
            </div>
          ) : null}
          {actionError ? (
            <p className="text-body-sm text-error">{actionError}</p>
          ) : null}
          <DialogFooter>
            <Button
              disabled={killing}
              onClick={() => setKillTarget(null)}
              variant="secondary"
            >
              Cancel
            </Button>
            <Button
              disabled={
                killing || killTarget?.sessionId === telemetry?.monitorSessionId
              }
              onClick={() => void confirmKill()}
              variant="danger"
            >
              {killing ? "Killing…" : "Kill session"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );

  function filterOption(value: SessionFilter, label: string) {
    return (
      <button
        type="button"
        aria-pressed={filter === value}
        className={`rounded px-2 py-1 text-label-sm ${filter === value ? "border border-primary/60 bg-primary-container/40 text-on-surface" : "text-on-surface-variant hover:bg-surface-subtle"}`}
        key={value}
        onClick={() => setFilter(value)}
      >
        {label}
      </button>
    );
  }
}

function UnavailableState({
  title,
  description,
  action,
}: {
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return (
    <Surface className="p-8">
      <div className="flex min-h-64 flex-col items-center justify-center text-center">
        <Icon name="activity" size={28} className="mb-3 text-primary" />
        <h2 className="text-body-md font-semibold text-on-surface">{title}</h2>
        <p className="mt-2 max-w-xl text-body-sm text-on-surface-variant">
          {description}
        </p>
        {action ? <div className="mt-4">{action}</div> : null}
      </div>
    </Surface>
  );
}

function Summary({
  label,
  value,
  tone = "neutral",
}: {
  label: string;
  value: number;
  tone?: "neutral" | "error" | "warning";
}) {
  return (
    <Surface className="flex items-center justify-between px-3 py-2">
      <span className="text-label-caps text-on-surface-variant">{label}</span>
      <StatusPill tone={tone}>{value}</StatusPill>
    </Surface>
  );
}

function SessionStatus({ session }: { session: DatabaseSession }) {
  const status = session.requestStatus ?? session.status;
  const tone =
    session.blockingSessionId && session.blockingSessionId > 0
      ? "error"
      : session.requestStatus
        ? "success"
        : "neutral";
  return <StatusPill tone={tone}>{status}</StatusPill>;
}

function SessionDetails({
  session,
  open,
  onClose,
  onKill,
  onSelectSession,
  monitorSessionId,
}: {
  session: DatabaseSession | null;
  open: boolean;
  onClose: () => void;
  onKill: () => void;
  onSelectSession: (sessionId: number) => void;
  monitorSessionId: number | null;
}) {
  const rootBlockerSessionId = session?.rootBlockerSessionId ?? null;
  const copySql = async () => {
    const sql = session?.currentStatement ?? session?.batchText;
    if (sql) await navigator.clipboard.writeText(sql);
  };
  return (
    <Drawer
      onOpenChange={(value) => !value && onClose()}
      open={open}
      swipeDirection="right"
    >
      <DrawerContent className="w-[min(560px,90vw)]">
        <DrawerHeader className="border-b border-outline-variant pb-4 text-left">
          <DrawerTitle>Session #{session?.sessionId}</DrawerTitle>
          <DrawerDescription>
            {session?.database ?? "Unknown database"} ·{" "}
            {session?.login ?? "Unknown login"}
          </DrawerDescription>
        </DrawerHeader>
        {session ? (
          <div className="min-h-0 flex-1 space-y-5 overflow-auto p-4">
            <section>
              <SectionTitle>Overview</SectionTitle>
              <div className="grid grid-cols-2 gap-3">
                {[
                  ["Status", session.requestStatus ?? session.status],
                  ["Command", session.command],
                  ["Duration", formatDuration(session.durationMs)],
                  ["CPU", `${formatCount(session.cpuTimeMs)} ms`],
                  ["Reads", formatCount(session.reads)],
                  ["Writes", formatCount(session.writes)],
                  ["Logical reads", formatCount(session.logicalReads)],
                  ["Rows", formatCount(session.rowCount)],
                  ["Open transactions", session.openTransactionCount],
                  ["Blocking sessions", session.blockedSessionCount],
                  ["Isolation", session.transactionIsolationName],
                ].map((entry) => (
                  <ContextValue
                    key={String(entry[0])}
                    label={String(entry[0])}
                    value={entry[1]}
                  />
                ))}
              </div>
            </section>
            <section>
              <div className="mb-2 flex items-center justify-between">
                <SectionTitle>Query</SectionTitle>
                <Button onClick={() => void copySql()} variant="secondary">
                  Copy SQL
                </Button>
              </div>
              {session.currentStatement ? (
                <SessionSqlViewer value={session.currentStatement} />
              ) : (
                <p className="rounded-md border border-outline-variant bg-surface-subtle p-3 text-body-sm text-on-surface-variant">
                  Current statement unavailable
                </p>
              )}
              {session.batchText &&
              session.batchText !== session.currentStatement ? (
                <div className="mt-3">
                  <p className="mb-1 text-label-caps text-on-surface-variant">
                    Full batch
                  </p>
                  <SessionSqlViewer value={session.batchText} />
                </div>
              ) : null}
              {session.percentComplete > 0 ? (
                <div className="mt-2 text-body-sm">
                  Progress {session.percentComplete.toFixed(1)}%
                  {BigInt(session.estimatedCompletionTimeMs) > 0n
                    ? ` · ${formatDuration(session.estimatedCompletionTimeMs)} remaining`
                    : ""}
                </div>
              ) : null}
            </section>
            <section>
              <SectionTitle>Waits &amp; blocking</SectionTitle>
              <div className="grid grid-cols-2 gap-3">
                <ContextValue
                  label="Wait type"
                  value={session.waitType ?? session.lastWaitType}
                />
                <ContextValue
                  label="Wait duration"
                  value={`${formatCount(session.waitTimeMs)} ms`}
                />
                <ContextValue
                  label="Wait resource"
                  value={session.waitResource}
                />
                <ContextValue
                  label="Blocked by"
                  value={
                    session.blockingSessionId
                      ? session.blockingSessionId > 0
                        ? `#${session.blockingSessionId}`
                        : `Resource ${session.blockingSessionId}`
                      : "No blocker"
                  }
                />
                <ContextValue
                  label="Blocker depth"
                  value={session.blockingDepth}
                />
                <div>
                  <div className="text-label-caps text-on-surface-variant">
                    Root blocker
                  </div>
                  {session.isRootBlocker ? (
                    <div className="text-body-sm text-on-surface">
                      This session
                    </div>
                  ) : rootBlockerSessionId !== null ? (
                    <button
                      className="font-mono text-body-sm text-primary underline"
                      onClick={() => onSelectSession(rootBlockerSessionId)}
                      type="button"
                    >
                      #{rootBlockerSessionId}
                    </button>
                  ) : (
                    <div className="text-body-sm text-on-surface">—</div>
                  )}
                </div>
              </div>
            </section>
            <section>
              <SectionTitle>Connection</SectionTitle>
              <div className="grid grid-cols-2 gap-3">
                {[
                  ["Login", session.login],
                  ["Original login", session.originalLogin],
                  ["Host", session.host],
                  ["Application", session.application],
                  ["Client interface", session.clientInterface],
                  ["Client address", session.clientAddress],
                  ["Local address", session.localAddress],
                  ["Local port", session.localPort],
                  ["Login time", session.loginTime],
                ].map((entry) => (
                  <ContextValue
                    key={String(entry[0])}
                    label={String(entry[0])}
                    value={entry[1]}
                  />
                ))}
              </div>
            </section>
          </div>
        ) : null}
        <div className="border-t border-outline-variant p-4">
          <Button
            disabled={!session || session.sessionId === monitorSessionId}
            onClick={onKill}
            variant="danger"
          >
            Kill session
          </Button>
        </div>
      </DrawerContent>
    </Drawer>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-2 font-mono text-label-caps font-semibold uppercase text-primary">
      {children}
    </h3>
  );
}

function ContextValue({
  label,
  value,
  className = "",
}: {
  label: string;
  value: unknown;
  className?: string;
}) {
  return (
    <div className={className}>
      <div className="text-label-caps text-on-surface-variant">{label}</div>
      <div className="break-words text-body-sm text-on-surface">
        {value === null || value === undefined || value === ""
          ? "—"
          : String(value)}
      </div>
    </div>
  );
}

function formatDuration(durationMs: string | number) {
  const seconds = BigInt(durationMs) / 1000n;
  const hours = seconds / 3600n;
  const minutes = (seconds % 3600n) / 60n;
  const remainder = seconds % 60n;
  if (hours) return `${hours}h ${minutes}m ${remainder}s`;
  if (minutes) return `${minutes}m ${remainder}s`;
  return `${remainder}s`;
}

function formatCount(value: number | string) {
  return new Intl.NumberFormat().format(
    typeof value === "string" ? BigInt(value) : value,
  );
}
