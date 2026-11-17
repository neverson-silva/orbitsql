import type {
  ConnectionStatus,
  ExplorerChild,
  ExplorerConnection,
} from "../../stores/connections.store";
import type { CatalogIndex } from "../sql-intelligence/catalog";
import type { DialectId } from "../sql-intelligence/dialect/types";

export type ContextDimensionId =
  | "catalog"
  | "database"
  | "schema"
  | "service"
  | "container"
  | "user";

export type ContextSelectionMode =
  | "session"
  | "reconnect"
  | "fixed"
  | "unsupported";

export type ContextDimensionDescriptor = Readonly<{
  id: ContextDimensionId;
  label: string;
  selectionMode: ContextSelectionMode;
  priority: number;
}>;

export type ExecutionContext = Readonly<{
  connectionId: string | null;
  dimensions: Readonly<Partial<Record<ContextDimensionId, string>>>;
  connectionState: ConnectionStatus;
}>;

const postgresCapabilities: readonly ContextDimensionDescriptor[] = [
  {
    id: "database",
    label: "Database",
    selectionMode: "reconnect",
    priority: 1,
  },
  { id: "schema", label: "Schema", selectionMode: "session", priority: 2 },
];

const mysqlCapabilities: readonly ContextDimensionDescriptor[] = [
  { id: "database", label: "Database", selectionMode: "session", priority: 1 },
];

const sqlServerCapabilities: readonly ContextDimensionDescriptor[] = [
  { id: "database", label: "Database", selectionMode: "session", priority: 1 },
];

const oracleCapabilities: readonly ContextDimensionDescriptor[] = [
  { id: "service", label: "Service", selectionMode: "reconnect", priority: 1 },
  {
    id: "container",
    label: "Container",
    selectionMode: "reconnect",
    priority: 2,
  },
  { id: "user", label: "User / schema", selectionMode: "session", priority: 3 },
];

const genericCapabilities: readonly ContextDimensionDescriptor[] = [
  { id: "database", label: "Database", selectionMode: "fixed", priority: 1 },
];

export function getExecutionContextCapabilities(
  dialect: DialectId,
): readonly ContextDimensionDescriptor[] {
  if (dialect === "postgresql") return postgresCapabilities;
  if (dialect === "mysql" || dialect === "mariadb") return mysqlCapabilities;
  if (dialect === "sqlserver") return sqlServerCapabilities;
  if (dialect === "oracle") return oracleCapabilities;
  return genericCapabilities;
}

export function dialectForConnection(
  connection?: ExplorerConnection,
): DialectId {
  if (connection?.engine === "PostgreSQL") return "postgresql";
  if (connection?.engine === "MySQL") return "mysql";
  if (connection?.engine === "MariaDB") return "mariadb";
  if (connection?.engine === "Microsoft SQL Server") return "sqlserver";
  if (connection?.engine === "Oracle Database") return "oracle";
  return "generic";
}

export function contextFromCatalog(
  connection: ExplorerConnection | undefined,
  catalog: CatalogIndex | undefined,
  previous: ExecutionContext | undefined,
): ExecutionContext {
  const dimensions = { ...(previous?.dimensions ?? {}) };
  const database = catalog?.snapshot.databaseId;
  const databaseName = catalog?.snapshot.databases.find(
    (item) => item.id === database,
  )?.name.raw;
  if (!dimensions.database) {
    dimensions.database = databaseName ?? connection?.detail.split(" • ")[1];
  }
  if (!("schema" in dimensions) && catalog?.snapshot.schemas.length === 1) {
    dimensions.schema = catalog.snapshot.schemas[0]?.name.raw;
  }
  return {
    connectionId: connection?.id ?? null,
    dimensions,
    connectionState: connection?.status ?? "disconnected",
  };
}

export function formatExecutionContext(
  context: ExecutionContext,
  capabilities: readonly ContextDimensionDescriptor[],
): string {
  const values = [...capabilities]
    .sort((left, right) => left.priority - right.priority)
    .map((dimension) => context.dimensions[dimension.id])
    .filter((value): value is string => Boolean(value));
  if (values.length < 2) return values[0] ?? "No context selected";
  const separator = capabilities.some((dimension) => dimension.id === "service")
    ? " · "
    : " @ ";
  return values.join(separator);
}

export function contextFromExplorerNode(
  connection: ExplorerConnection,
  child: ExplorerChild | undefined,
  catalog: CatalogIndex | undefined,
) {
  const context = contextFromCatalog(connection, catalog, undefined);
  if (!child) return context;
  const dimensions = { ...context.dimensions };
  if (child.kind === "database") dimensions.database = child.name;
  if (child.kind === "schema") dimensions.schema = child.name;
  if (child.kind === "user") dimensions.user = child.name;
  let parentId = child.parentId;
  while (parentId) {
    const parent = connection.children.find(
      (candidate) => candidate.id === parentId,
    );
    if (!parent) break;
    if (parent.kind === "database") dimensions.database = parent.name;
    if (parent.kind === "schema") dimensions.schema = parent.name;
    parentId = parent.parentId;
  }
  return { ...context, dimensions };
}

export function contextSwitchSql(
  dialect: DialectId,
  dimension: ContextDimensionId,
  value: string,
): string | null {
  const escaped = `"${value.replace(/"/g, '""')}"`;
  if (dimension === "schema" && dialect === "postgresql") {
    return `SET search_path TO ${escaped};`;
  }
  if (
    dimension === "database" &&
    (dialect === "mysql" || dialect === "mariadb")
  ) {
    return `USE ${escaped};`;
  }
  if (dimension === "database" && dialect === "sqlserver") {
    return `USE ${escaped};`;
  }
  if (dimension === "user" && dialect === "oracle") {
    return `ALTER SESSION SET CURRENT_SCHEMA = ${escaped};`;
  }
  return null;
}

export function executionContextSetupSql(
  dialect: DialectId,
  context: ExecutionContext,
): string | null {
  if (dialect === "postgresql" && context.dimensions.schema) {
    return contextSwitchSql(dialect, "schema", context.dimensions.schema);
  }
  if (
    (dialect === "mysql" || dialect === "mariadb" || dialect === "sqlserver") &&
    context.dimensions.database
  ) {
    return contextSwitchSql(dialect, "database", context.dimensions.database);
  }
  if (dialect === "oracle" && context.dimensions.user) {
    return contextSwitchSql(dialect, "user", context.dimensions.user);
  }
  return null;
}
