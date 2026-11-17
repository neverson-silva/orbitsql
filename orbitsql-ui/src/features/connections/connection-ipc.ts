import { invoke } from "@tauri-apps/api/core";
import { useConnectionsStore } from "../../stores/connections.store";
import type { CatalogSnapshot } from "../sql-intelligence/catalog";

export type ConnectionDialect = "postgresql" | "sqlserver";
export type ConnectionAuthMethod = "password" | "windowsNtlm";

export type ConnectionTransport = Readonly<{
  mode: "direct" | "ssh" | "proxy";
  sshHost: string;
  sshPort: number;
  sshUserName: string;
  sshPrivateKeyPath: string;
  proxyHost: string;
  proxyPort: number;
  proxyType: "socks5" | "httpConnect";
}>;

export type ConnectionDraft = Readonly<{
  name: string;
  dialect: ConnectionDialect;
  server: string;
  port: number;
  userName: string;
  password: string;
  database: string;
  trustServerCertificate: boolean;
  authMethod: ConnectionAuthMethod;
  profileId: string;
  showAllCatalogItems: boolean;
  transport: ConnectionTransport;
  driverProperties: Readonly<Record<string, string>>;
  keepAliveEnabled: boolean;
  keepAliveIntervalSeconds: number;
  idleTimeoutSeconds: number | null;
}>;

export type ConnectionUpdateDraft = Readonly<{
  id: string;
  name: string;
  dialect: ConnectionDialect;
  server: string;
  port: number;
  userName: string;
  password: string | null;
  database: string;
  trustServerCertificate: boolean;
  authMethod: ConnectionAuthMethod;
  profileId: string;
  showAllCatalogItems: boolean;
  keepAliveEnabled: boolean;
  keepAliveIntervalSeconds: number;
  idleTimeoutSeconds: number | null;
}>;

export type ConnectionDiagnostic = Readonly<{
  success: boolean;
  latencyMs: number;
  serverVersion: string | null;
  database: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  recoverable: boolean;
  capabilities: DatabaseCapabilities | null;
}>;

export type DatabaseCapabilities = Readonly<{
  databases: boolean;
  schemas: boolean;
  tables: boolean;
  views: boolean;
  materializedViews: boolean;
  procedures: boolean;
  functions: boolean;
  triggers: boolean;
  sequences: boolean;
  extensions: boolean;
  partitions: boolean;
  users: boolean;
  roles: boolean;
  permissions: boolean;
  synonyms: boolean;
  indexes: boolean;
  foreignKeys: boolean;
  customTypes: boolean;
}>;

export type SavedConnectionProfile = Readonly<{
  id: string;
  name: string;
  dialect: ConnectionDialect;
  server: string;
  port: number;
  userName: string;
  database: string;
  trustServerCertificate: boolean;
  authMethod: ConnectionAuthMethod;
  profileId: string;
  showAllCatalogItems: boolean;
  createdAt: string;
  updatedAt: string;
  keepAliveEnabled: boolean;
  keepAliveIntervalSeconds: number;
  idleTimeoutSeconds: number | null;
}>;

export type ConnectionProfileDefinition = Readonly<{
  id: string;
  name: string;
  color: string;
  description: string;
}>;

export type ConnectionSessionResult = Readonly<{
  profile: SavedConnectionProfile;
  catalog: CatalogSnapshot;
  capabilities: DatabaseCapabilities;
}>;

export type CatalogScope =
  | Readonly<{ databaseId: string }>
  | Readonly<{ schemaId: string; databaseId?: string }>
  | Readonly<{ tableId: string; databaseId?: string }>;

export type QueryColumn = Readonly<{
  name: string;
  typeName: string;
}>;

type QueryResultBase = Readonly<{
  columns: QueryColumn[];
  rows: unknown[][];
  rowCount: number;
  affectedRows: number | null;
  elapsedMs: number;
}>;

export type LegacyQueryResult = QueryResultBase;

export type QueryResult = QueryResultBase &
  Readonly<{
    sessionId: string | null;
    hasMore: boolean;
    totalFetched: number;
    rowLimitReached: boolean;
  }>;

export type QueryResultBatch = Readonly<{
  rows: unknown[][];
  rowCount: number;
  hasMore: boolean;
  totalFetched: number;
}>;

export type QueryResultSessionStart = Readonly<{
  sessionId: string;
  columns: QueryColumn[];
  firstBatch: QueryResultBatch;
  elapsedMs: number;
}>;

export type DatabaseSession = Readonly<{
  sessionId: number;
  status: string;
  requestStatus: string | null;
  database: string | null;
  login: string | null;
  originalLogin: string | null;
  host: string | null;
  application: string | null;
  clientInterface: string | null;
  clientAddress: string | null;
  localAddress: string | null;
  localPort: number | null;
  loginTime: string | null;
  command: string | null;
  durationMs: string;
  cpuTimeMs: number;
  reads: string;
  writes: string;
  logicalReads: string;
  rowCount: string;
  waitType: string | null;
  waitTimeMs: number;
  waitResource: string | null;
  lastWaitType: string | null;
  blockingSessionId: number | null;
  blockerSessionId: number | null;
  rootBlockerSessionId: number | null;
  isBlocking: boolean;
  isRootBlocker: boolean;
  blockedSessionCount: number;
  blockingDepth: number;
  openTransactionCount: number;
  transactionIsolationLevel: number;
  transactionIsolationName: string;
  batchText: string | null;
  currentStatement: string | null;
  percentComplete: number;
  estimatedCompletionTimeMs: string;
}>;

export type SessionTelemetry = Readonly<{
  sessions: DatabaseSession[];
  monitorSessionId: number;
  capturedAt: string;
}>;

export type SessionKillResult = Readonly<{
  sessionId: number;
  message: string;
}>;

export function connectionErrorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null) {
    const candidate = error as { code?: unknown; message?: unknown };
    if (typeof candidate.message === "string") {
      return typeof candidate.code === "string"
        ? `${candidate.code}: ${candidate.message}`
        : candidate.message;
    }
  }
  return error instanceof Error ? error.message : String(error);
}

export function isConnectionReconnectFailure(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code?: unknown }).code === "CONNECTION_RECONNECT_FAILED"
  );
}

export async function testConnection(
  draft: ConnectionDraft,
  connectionId?: string,
): Promise<ConnectionDiagnostic> {
  return invoke<ConnectionDiagnostic>("test_connection", {
    draft,
    connectionId: connectionId ?? null,
  });
}

export async function saveConnection(
  draft: ConnectionDraft,
): Promise<ConnectionSessionResult> {
  return invoke<ConnectionSessionResult>("save_connection", { draft });
}

export async function saveConnectionProfile(
  draft: ConnectionDraft,
): Promise<SavedConnectionProfile> {
  return invoke<SavedConnectionProfile>("save_connection_profile", { draft });
}

export async function listSavedConnections(): Promise<
  SavedConnectionProfile[]
> {
  return invoke<SavedConnectionProfile[]>("list_connection_profiles");
}

export async function listConnectionProfileDefinitions(): Promise<
  ConnectionProfileDefinition[]
> {
  return invoke<ConnectionProfileDefinition[]>(
    "list_connection_profile_definitions",
  );
}

export async function upsertConnectionProfileDefinition(
  profile: ConnectionProfileDefinition,
): Promise<ConnectionProfileDefinition> {
  return invoke<ConnectionProfileDefinition>(
    "upsert_connection_profile_definition",
    {
      profile,
    },
  );
}

export async function deleteConnectionProfileDefinition(
  profileId: string,
): Promise<void> {
  return invoke<void>("delete_connection_profile_definition", { profileId });
}

export async function listConnectionStatuses(): Promise<string[]> {
  return invoke<string[]>("list_connection_statuses");
}

export async function pingConnection(connectionId: string): Promise<boolean> {
  return invoke<boolean>("ping_connection", { connectionId });
}

export async function connectConnection(
  connectionId: string,
): Promise<ConnectionSessionResult> {
  return invoke<ConnectionSessionResult>("connect_connection", {
    connectionId,
  });
}

export async function connectConnectionOnly(
  connectionId: string,
): Promise<SavedConnectionProfile> {
  return invoke<SavedConnectionProfile>("connect_connection_only", {
    connectionId,
  });
}

export async function disconnectConnection(
  connectionId: string,
): Promise<void> {
  return invoke<void>("disconnect_connection", { connectionId });
}

export async function updateConnection(
  draft: ConnectionUpdateDraft,
  reconnect: boolean,
): Promise<SavedConnectionProfile> {
  return invoke<SavedConnectionProfile>("update_connection", {
    draft,
    reconnect,
  });
}

export async function deleteConnection(connectionId: string): Promise<void> {
  return invoke<void>("delete_connection", { connectionId });
}

export async function refreshConnectionCatalog(
  connectionId: string,
): Promise<CatalogSnapshot> {
  try {
    return await invoke<CatalogSnapshot>("refresh_connection_catalog", {
      connectionId,
      version: new Date().toISOString(),
      generatedAt: new Date().toISOString(),
    });
  } catch (error) {
    if (isConnectionReconnectFailure(error)) {
      const connections = useConnectionsStore.getState();
      connections.resetConnectionTree(connectionId);
      connections.setConnectionStatus(connectionId, "disconnected");
    }
    throw error;
  }
}

export async function expandConnectionNode(
  connectionId: string,
  scope: CatalogScope,
): Promise<CatalogSnapshot> {
  return invoke<CatalogSnapshot>("expand_connection_node", {
    connectionId,
    scope,
  });
}

export async function renameConnection(
  connectionId: string,
  name: string,
): Promise<SavedConnectionProfile> {
  return invoke<SavedConnectionProfile>("rename_connection", {
    connectionId,
    name,
  });
}

export async function executeQuery(
  connectionId: string,
  sql: string,
): Promise<LegacyQueryResult> {
  return invoke<LegacyQueryResult>("execute_query", { connectionId, sql });
}

export async function executeQueryInContext(
  connectionId: string,
  sql: string,
  context: { database?: string; schema?: string },
): Promise<LegacyQueryResult> {
  return invoke<LegacyQueryResult>("execute_query_in_context", {
    connectionId,
    sql,
    database: context.database ?? null,
    schema: context.schema ?? null,
  });
}

export async function getDatabaseSessions(
  connectionId: string,
): Promise<SessionTelemetry> {
  return invoke<SessionTelemetry>("get_database_sessions", { connectionId });
}

export async function killDatabaseSession(
  connectionId: string,
  sessionId: number,
): Promise<SessionKillResult> {
  return invoke<SessionKillResult>("kill_database_session", {
    connectionId,
    sessionId,
  });
}

export async function startQuerySessionInContext(
  connectionId: string,
  sql: string,
  context: { database?: string; schema?: string },
  initialRows = 200,
): Promise<QueryResultSessionStart> {
  return invoke<QueryResultSessionStart>("start_query_session_in_context", {
    connectionId,
    sql,
    database: context.database ?? null,
    schema: context.schema ?? null,
    initialRows,
  });
}

export async function fetchQuerySession(
  sessionId: string,
  requestedRows = 200,
): Promise<QueryResultBatch> {
  return invoke<QueryResultBatch>("fetch_query_session", {
    sessionId,
    requestedRows,
  });
}

export async function cancelQuerySession(sessionId: string): Promise<void> {
  return invoke<void>("cancel_query_session", { sessionId });
}

export async function closeQuerySession(sessionId: string): Promise<void> {
  return invoke<void>("close_query_session", { sessionId });
}

export async function cancelQuery(connectionId: string): Promise<boolean> {
  return invoke<boolean>("cancel_query", { connectionId });
}

export async function secretStoreStatus(): Promise<string> {
  return invoke<string>("secret_store_status");
}

export async function createSecretVault(masterPassword: string): Promise<void> {
  return invoke<void>("create_secret_vault", { masterPassword });
}

export async function unlockSecretVault(masterPassword: string): Promise<void> {
  return invoke<void>("unlock_secret_vault", { masterPassword });
}
