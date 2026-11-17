import type {
  ExplorerChild,
  ExplorerChildKind,
  ExplorerGroupKind,
} from "../../stores/connections.store";
import type { CatalogSnapshot } from "../sql-intelligence/catalog";

function sortByName<T extends { name: { raw: string } }>(items: readonly T[]) {
  return [...items].sort((left, right) => {
    const comparison = left.name.raw.localeCompare(right.name.raw, undefined, {
      sensitivity: "base",
    });
    return comparison || left.name.raw.localeCompare(right.name.raw);
  });
}

function sortExplorerChildren(children: readonly ExplorerChild[]) {
  return [...children].sort(
    (left, right) =>
      left.name.localeCompare(right.name, undefined, { sensitivity: "base" }) ||
      left.name.localeCompare(right.name),
  );
}

export function childrenFromSnapshot(
  snapshot: CatalogSnapshot,
  namespace?: string,
): ExplorerChild[] {
  if (snapshot.databaseId === "" && snapshot.databases.length > 0) {
    return sortByName(snapshot.databases).map((database) => ({
      id: database.id,
      kind: "database" as const,
      name: database.name.raw,
      parentId: null,
      expanded: false,
      loaded: false,
      databaseId: database.name.raw,
    }));
  }

  const schemas = sortByName(snapshot.schemas).map((schema) => ({
    id: namespace ? `${namespace}:${schema.id}` : schema.id,
    kind: "schema" as const,
    name: schema.name.raw,
    parentId: null,
    expanded: false,
    loaded: false,
    databaseId: schema.databaseId,
    remoteId: namespace ? schema.id : undefined,
  }));
  const tables = sortByName(snapshot.tables).map((table) => ({
    id: namespace ? `${namespace}:${table.id}` : table.id,
    kind: "table" as const,
    name: table.name.raw,
    parentId: null,
    expanded: false,
    loaded: table.columns.length > 0,
    databaseId: table.databaseId,
    remoteId: namespace ? table.id : undefined,
  }));
  return [...schemas, ...(schemas.length === 0 ? tables : [])];
}

type GroupDefinition = {
  key: ExplorerGroupKind;
  label: string;
  leafKind: ExplorerChildKind;
};

const schemaGroups: readonly GroupDefinition[] = [
  { key: "tables", label: "Tables", leafKind: "table" },
  { key: "views", label: "Views", leafKind: "view" },
  { key: "procedures", label: "Procedures", leafKind: "procedure" },
  {
    key: "foreignTables",
    label: "Foreign Tables",
    leafKind: "foreignTable",
  },
  {
    key: "materializedViews",
    label: "Materialized Views",
    leafKind: "materializedView",
  },
  { key: "indexes", label: "Indexes", leafKind: "index" },
  { key: "functions", label: "Functions", leafKind: "function" },
  { key: "sequences", label: "Sequences", leafKind: "sequence" },
  { key: "dataTypes", label: "Data Types", leafKind: "dataType" },
  {
    key: "aggregateFunctions",
    label: "Aggregate Functions",
    leafKind: "function",
  },
];

function explorerLeaf(
  id: string,
  kind: ExplorerChildKind,
  name: string,
  parentId: string,
  remoteId?: string,
  loaded = true,
): ExplorerChild {
  return {
    id,
    kind,
    name,
    parentId,
    expanded: false,
    loaded,
    ...(remoteId ? { remoteId } : {}),
  };
}

export function schemaChildrenFromSnapshot(
  snapshot: CatalogSnapshot,
  schemaNodeId: string,
  schemaId: string,
): ExplorerChild[] {
  const schemaTables = snapshot.tables.filter(
    (item) => item.schemaId === schemaId,
  );
  const tables = sortByName(schemaTables.filter((item) => !item.foreign));
  const foreignTables = sortByName(schemaTables.filter((item) => item.foreign));
  const tableIds = new Set(schemaTables.map((item) => item.id));
  const views = sortByName(
    snapshot.views.filter((item) => item.schemaId === schemaId),
  );
  const materializedViews = sortByName(
    snapshot.materializedViews.filter((item) => item.schemaId === schemaId),
  );
  const procedures = sortByName(
    snapshot.procedures.filter((item) => item.schemaId === schemaId),
  );
  const functions = sortByName(
    snapshot.functions.filter(
      (item) => item.schemaId === schemaId && item.kind !== "aggregate",
    ),
  );
  const aggregateFunctions = sortByName(
    snapshot.functions.filter(
      (item) => item.schemaId === schemaId && item.kind === "aggregate",
    ),
  );
  const sequences = sortByName(
    snapshot.sequences.filter((item) => item.schemaId === schemaId),
  );
  const dataTypes = sortByName(
    snapshot.types.filter((item) => item.schemaId === schemaId),
  );
  const indexes = sortByName(
    snapshot.indexes.filter((item) => tableIds.has(item.tableId)),
  );

  const groups: Record<ExplorerGroupKind, ExplorerChild[]> = {
    tables: tables.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:tables:${item.id}`,
        "table",
        item.name.raw,
        `${schemaNodeId}:group:tables`,
        item.id,
        false,
      ),
    ),
    views: views.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:views:${item.id}`,
        "view",
        item.name.raw,
        `${schemaNodeId}:group:views`,
        item.id,
      ),
    ),
    procedures: procedures.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:procedures:${item.id}`,
        "procedure",
        item.name.raw,
        `${schemaNodeId}:group:procedures`,
        item.id,
      ),
    ),
    foreignTables: foreignTables.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:foreign-tables:${item.id}`,
        "foreignTable",
        item.name.raw,
        `${schemaNodeId}:group:foreignTables`,
        item.id,
      ),
    ),
    materializedViews: materializedViews.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:materialized-views:${item.id}`,
        "materializedView",
        item.name.raw,
        `${schemaNodeId}:group:materializedViews`,
        item.id,
      ),
    ),
    indexes: indexes.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:indexes:${item.id}`,
        "index",
        item.name.raw,
        `${schemaNodeId}:group:indexes`,
        item.id,
      ),
    ),
    functions: functions.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:functions:${item.id}`,
        "function",
        item.name.raw,
        `${schemaNodeId}:group:functions`,
        item.id,
      ),
    ),
    sequences: sequences.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:sequences:${item.id}`,
        "sequence",
        item.name.raw,
        `${schemaNodeId}:group:sequences`,
        item.id,
      ),
    ),
    dataTypes: dataTypes.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:data-types:${item.id}`,
        "dataType",
        item.name.raw,
        `${schemaNodeId}:group:dataTypes`,
        item.id,
      ),
    ),
    aggregateFunctions: aggregateFunctions.map((item) =>
      explorerLeaf(
        `${schemaNodeId}:aggregate-functions:${item.id}`,
        "function",
        item.name.raw,
        `${schemaNodeId}:group:aggregateFunctions`,
        item.id,
      ),
    ),
  };

  return schemaGroups.flatMap((definition) => {
    const groupId = `${schemaNodeId}:group:${definition.key}`;
    return [
      {
        id: groupId,
        kind: "group" as const,
        group: definition.key,
        name: `${definition.label} (${groups[definition.key].length})`,
        parentId: schemaNodeId,
        expanded: false,
        loaded: true,
      },
      ...sortExplorerChildren(groups[definition.key]),
    ];
  });
}
