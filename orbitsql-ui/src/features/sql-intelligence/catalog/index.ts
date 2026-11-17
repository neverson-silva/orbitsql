import type { CompletionExecutionContext } from "../completion/types";
import type {
  CatalogFunction,
  CatalogIndex,
  CatalogObject,
  CatalogSnapshot,
  CatalogTable,
  CatalogType,
  CatalogView,
  ForeignKeyMetadata,
  Identifier,
  QualifiedName,
} from "./types";

export * from "./contract";
export * from "./types";

export function identifierKey(identifier: Identifier | string): string {
  return typeof identifier === "string"
    ? identifier.toLocaleLowerCase()
    : identifier.normalized.toLocaleLowerCase();
}

export function qualifiedNameKey(
  parts: readonly (Identifier | string | undefined)[],
): string {
  return parts
    .filter((part): part is Identifier | string => part !== undefined)
    .map(identifierKey)
    .join(".");
}

export function buildCatalogIndex(snapshot: CatalogSnapshot): CatalogIndex {
  const schemasById = new Map<string, CatalogSnapshot["schemas"][number]>();
  const schemasByName = new Map<string, CatalogSnapshot["schemas"][number]>();
  const objectsByQualifiedName = new Map<string, CatalogObject>();
  const objectsByUnqualifiedName = new Map<string, CatalogObject[]>();
  const objectsByPrefix = new Map<string, CatalogObject[]>();
  const columnsByTableId = new Map<
    string,
    CatalogSnapshot["tables"][number]["columns"]
  >();
  const relationsByTableId = new Map<string, ForeignKeyMetadata[]>();
  const functionsByName = new Map<string, CatalogFunction[]>();
  const functionsByPrefix = new Map<string, CatalogFunction[]>();
  const typesByName = new Map<string, CatalogType[]>();
  const typesByPrefix = new Map<string, CatalogType[]>();
  const schemaNames = new Map<string, CatalogSnapshot["schemas"][number]>();

  for (const schema of snapshot.schemas) {
    schemasById.set(schema.id, schema);
    schemasByName.set(identifierKey(schema.name), schema);
    schemaNames.set(schema.id, schema);
  }

  const addObject = (object: CatalogTable | CatalogView) => {
    const schema = schemaNames.get(object.schemaId);
    const keys = [
      qualifiedNameKey([schema?.name, object.name]),
      qualifiedNameKey([schema?.catalogId, schema?.name, object.name]),
      qualifiedNameKey([object.catalogId, schema?.name, object.name]),
      qualifiedNameKey([object.databaseId, schema?.name, object.name]),
    ];
    for (const key of new Set(keys)) objectsByQualifiedName.set(key, object);
    const unqualifiedKey = identifierKey(object.name);
    const current = objectsByUnqualifiedName.get(unqualifiedKey) ?? [];
    current.push(object);
    objectsByUnqualifiedName.set(unqualifiedKey, current);
    for (const prefix of prefixes(unqualifiedKey)) {
      const prefixed = objectsByPrefix.get(prefix) ?? [];
      prefixed.push(object);
      objectsByPrefix.set(prefix, prefixed);
    }
    columnsByTableId.set(object.id, object.columns);
  };

  for (const table of snapshot.tables) {
    addObject(table);
  }
  for (const view of [...snapshot.views, ...snapshot.materializedViews]) {
    addObject(view);
  }

  for (const relation of snapshot.foreignKeys) {
    for (const tableId of [relation.sourceTableId, relation.targetTableId]) {
      const current = relationsByTableId.get(tableId) ?? [];
      current.push(relation);
      relationsByTableId.set(tableId, current);
    }
  }

  for (const fn of snapshot.functions) {
    const key = identifierKey(fn.name);
    const current = functionsByName.get(key) ?? [];
    current.push(fn);
    functionsByName.set(key, current);
    for (const prefix of prefixes(key)) {
      const prefixed = functionsByPrefix.get(prefix) ?? [];
      prefixed.push(fn);
      functionsByPrefix.set(prefix, prefixed);
    }
  }

  for (const type of snapshot.types) {
    const key = identifierKey(type.name);
    const current = typesByName.get(key) ?? [];
    current.push(type);
    typesByName.set(key, current);
    for (const prefix of prefixes(key)) {
      const prefixed = typesByPrefix.get(prefix) ?? [];
      prefixed.push(type);
      typesByPrefix.set(prefix, prefixed);
    }
  }

  return {
    snapshot,
    schemasById,
    schemasByName,
    objectsByQualifiedName,
    objectsByUnqualifiedName,
    objectsByPrefix,
    columnsByTableId,
    relationsByTableId,
    functionsByName,
    functionsByPrefix,
    typesByName,
    typesByPrefix,
  };
}

function prefixes(value: string): readonly string[] {
  const result: string[] = [];
  for (let length = 1; length <= value.length; length += 1) {
    result.push(value.slice(0, length));
  }
  return result;
}

export function findObject(
  index: CatalogIndex,
  name: string | QualifiedName,
  schema?: string,
): CatalogObject | undefined {
  if (typeof name !== "string") {
    const qualified = index.objectsByQualifiedName.get(
      qualifiedNameKey([name.catalog, name.schema, name.object]),
    );
    if (qualified) return qualified;
    return name.schema
      ? index.objectsByQualifiedName.get(
          qualifiedNameKey([name.schema, name.object]),
        )
      : index.objectsByUnqualifiedName.get(identifierKey(name.object))?.[0];
  }
  if (schema) {
    return index.objectsByQualifiedName.get(qualifiedNameKey([schema, name]));
  }
  return index.objectsByUnqualifiedName.get(identifierKey(name))?.[0];
}

export function findObjectInExecutionContext(
  index: CatalogIndex,
  name: string | QualifiedName,
  context?: CompletionExecutionContext,
): CatalogObject | undefined {
  if (typeof name !== "string" || !context) return findObject(index, name);

  const schemaName = context.schema ?? context.user;
  const databaseName = context.database ?? context.catalog;
  const database = databaseName
    ? index.snapshot.databases.find(
        (item) =>
          identifierKey(item.name) === identifierKey(databaseName) ||
          identifierKey(item.id) === identifierKey(databaseName),
      )
    : undefined;
  const schemaIds = new Set(
    index.snapshot.schemas
      .filter((schema) => {
        if (
          schemaName &&
          identifierKey(schema.name) !== identifierKey(schemaName)
        )
          return false;
        return !database || schema.databaseId === database.id;
      })
      .map((schema) => schema.id),
  );
  const objects = index.objectsByUnqualifiedName.get(identifierKey(name)) ?? [];
  return objects.find((object) => {
    if (schemaName && schemaIds.size > 0 && !schemaIds.has(object.schemaId))
      return false;
    if (database && object.databaseId !== database.id) return false;
    return true;
  });
}

export function createEmptyCatalogIndex(
  dialect: CatalogSnapshot["dialect"],
): CatalogIndex {
  return buildCatalogIndex({
    schemaVersion: 1,
    id: "catalog:empty",
    connectionId: "connection:none",
    databaseId: "database:none",
    dialect,
    version: "empty",
    generatedAt: new Date(0).toISOString(),
    databases: [],
    schemas: [],
    tables: [],
    views: [],
    materializedViews: [],
    functions: [],
    procedures: [],
    sequences: [],
    types: [],
    triggers: [],
    extensions: [],
    partitions: [],
    principals: [],
    permissions: [],
    synonyms: [],
    foreignKeys: [],
    indexes: [],
    constraints: [],
  });
}
