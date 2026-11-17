import type { CatalogSnapshot, SqlTypeFamily } from "./types";

export const catalogSchemaVersion = 1;

const dialects = new Set([
  "postgresql",
  "mysql",
  "mariadb",
  "sqlserver",
  "oracle",
  "sqlite",
  "generic",
]);

const typeFamilies = new Set<SqlTypeFamily>([
  "unknown",
  "null",
  "boolean",
  "integer",
  "decimal",
  "float",
  "text",
  "binary",
  "date",
  "time",
  "timestamp",
  "interval",
  "uuid",
  "json",
  "xml",
  "array",
  "enum",
  "spatial",
  "custom",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function requireString(value: unknown, path: string): void {
  if (typeof value !== "string") throw new Error(`${path} must be a string`);
}

function requireBoolean(value: unknown, path: string): void {
  if (typeof value !== "boolean") throw new Error(`${path} must be a boolean`);
}

function requireNumber(value: unknown, path: string): void {
  if (typeof value !== "number" || !Number.isFinite(value))
    throw new Error(`${path} must be a number`);
}

function requireStringArray(value: unknown, path: string): void {
  if (
    !Array.isArray(value) ||
    !value.every((entry) => typeof entry === "string")
  )
    throw new Error(`${path} must be an array of strings`);
}

function requireArray(
  value: unknown,
  path: string,
  validate: (entry: unknown, entryPath: string) => void,
): void {
  if (!Array.isArray(value)) throw new Error(`${path} must be an array`);
  value.forEach((entry, index) => {
    validate(entry, `${path}[${index}]`);
  });
}

function validateIdentifier(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.raw, `${path}.raw`);
  requireString(value.normalized, `${path}.normalized`);
  requireBoolean(value.quoted, `${path}.quoted`);
  if (
    value.quoteStyle !== "none" &&
    value.quoteStyle !== "double" &&
    value.quoteStyle !== "backtick" &&
    value.quoteStyle !== "bracket"
  )
    throw new Error(`${path}.quoteStyle is invalid`);
}

function validateType(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  if (!typeFamilies.has(value.family as SqlTypeFamily))
    throw new Error(`${path}.family is invalid`);
  requireString(value.name, `${path}.name`);
  if (value.nullable !== undefined)
    requireBoolean(value.nullable, `${path}.nullable`);
  if (value.precision !== undefined)
    requireNumber(value.precision, `${path}.precision`);
  if (value.scale !== undefined) requireNumber(value.scale, `${path}.scale`);
  if (value.elementType !== undefined)
    validateType(value.elementType, `${path}.elementType`);
}

function validateColumn(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  requireString(value.tableId, `${path}.tableId`);
  requireNumber(value.ordinal, `${path}.ordinal`);
  validateIdentifier(value.name, `${path}.name`);
  validateType(value.type, `${path}.type`);
  requireBoolean(value.nullable, `${path}.nullable`);
  requireBoolean(value.generated, `${path}.generated`);
  requireBoolean(value.identity, `${path}.identity`);
  requireBoolean(value.computed, `${path}.computed`);
  if (value.defaultValue !== undefined)
    requireString(value.defaultValue, `${path}.defaultValue`);
  requireBoolean(value.primaryKey, `${path}.primaryKey`);
  requireStringArray(
    value.foreignKeyMemberships,
    `${path}.foreignKeyMemberships`,
  );
  requireStringArray(value.uniqueMemberships, `${path}.uniqueMemberships`);
  if (value.comment !== undefined)
    requireString(value.comment, `${path}.comment`);
}

function validateForeignKey(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  requireString(value.sourceTableId, `${path}.sourceTableId`);
  requireStringArray(value.sourceColumnIds, `${path}.sourceColumnIds`);
  requireString(value.targetTableId, `${path}.targetTableId`);
  requireStringArray(value.targetColumnIds, `${path}.targetColumnIds`);
}

function validateIndex(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  requireString(value.tableId, `${path}.tableId`);
  requireStringArray(value.columnIds, `${path}.columnIds`);
  requireBoolean(value.unique, `${path}.unique`);
  requireBoolean(value.primary, `${path}.primary`);
}

function validateConstraint(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  requireString(value.tableId, `${path}.tableId`);
  if ("columnIds" in value)
    requireStringArray(value.columnIds, `${path}.columnIds`);
  if ("expression" in value && value.expression !== undefined)
    requireString(value.expression, `${path}.expression`);
}

function validateTable(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  requireString(value.databaseId, `${path}.databaseId`);
  requireString(value.schemaId, `${path}.schemaId`);
  validateIdentifier(value.name, `${path}.name`);
  requireArray(value.columns, `${path}.columns`, validateColumn);
  if (value.primaryKey !== undefined)
    validateConstraint(value.primaryKey, `${path}.primaryKey`);
  requireArray(value.foreignKeys, `${path}.foreignKeys`, validateForeignKey);
  requireArray(
    value.uniqueConstraints,
    `${path}.uniqueConstraints`,
    validateConstraint,
  );
  requireArray(
    value.checkConstraints,
    `${path}.checkConstraints`,
    validateConstraint,
  );
  requireArray(value.indexes, `${path}.indexes`, validateIndex);
  if (value.estimatedRows !== undefined)
    requireNumber(value.estimatedRows, `${path}.estimatedRows`);
  if (value.comment !== undefined)
    requireString(value.comment, `${path}.comment`);
  requireBoolean(value.temporary, `${path}.temporary`);
}

function validateView(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  requireString(value.databaseId, `${path}.databaseId`);
  requireString(value.schemaId, `${path}.schemaId`);
  validateIdentifier(value.name, `${path}.name`);
  requireArray(value.columns, `${path}.columns`, validateColumn);
  if (value.comment !== undefined)
    requireString(value.comment, `${path}.comment`);
}

function validateParameter(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  validateType(value.type, `${path}.type`);
  if (
    value.direction !== "in" &&
    value.direction !== "out" &&
    value.direction !== "inout"
  )
    throw new Error(`${path}.direction is invalid`);
  requireNumber(value.ordinal, `${path}.ordinal`);
}

function validateFunction(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  requireArray(value.parameters, `${path}.parameters`, validateParameter);
  validateType(value.returnType, `${path}.returnType`);
  requireBoolean(value.variadic, `${path}.variadic`);
  requireStringArray(value.overloads, `${path}.overloads`);
}

function validateProcedure(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  requireArray(value.parameters, `${path}.parameters`, validateParameter);
}

function validateSchema(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  requireString(value.databaseId, `${path}.databaseId`);
  validateIdentifier(value.name, `${path}.name`);
}

function validateDatabase(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  requireBoolean(value.system, `${path}.system`);
}

function validateTrigger(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  if (value.schemaId !== undefined) requireString(value.schemaId, `${path}.schemaId`);
  if (value.tableId !== undefined) requireString(value.tableId, `${path}.tableId`);
  validateIdentifier(value.name, `${path}.name`);
  requireBoolean(value.enabled, `${path}.enabled`);
  if (value.definition !== undefined) requireString(value.definition, `${path}.definition`);
}

function validateExtension(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  if (value.version !== undefined) requireString(value.version, `${path}.version`);
}

function validatePartition(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  requireString(value.tableId, `${path}.tableId`);
  if (value.name !== undefined) validateIdentifier(value.name, `${path}.name`);
  if (value.parentId !== undefined) requireString(value.parentId, `${path}.parentId`);
  if (value.bound !== undefined) requireString(value.bound, `${path}.bound`);
}

function validatePrincipal(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  requireString(value.kind, `${path}.kind`);
  if (value.login !== undefined) requireString(value.login, `${path}.login`);
}

function validatePermission(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  requireString(value.principalId, `${path}.principalId`);
  if (value.objectId !== undefined) requireString(value.objectId, `${path}.objectId`);
  requireString(value.permission, `${path}.permission`);
  requireString(value.state, `${path}.state`);
}

function validateSynonym(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  requireString(value.schemaId, `${path}.schemaId`);
  validateIdentifier(value.name, `${path}.name`);
  requireString(value.target, `${path}.target`);
}

function validateSequence(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  requireString(value.schemaId, `${path}.schemaId`);
  validateIdentifier(value.name, `${path}.name`);
  validateType(value.type, `${path}.type`);
}

function validateTypeEntry(value: unknown, path: string): void {
  if (!isRecord(value)) throw new Error(`${path} must be an object`);
  requireString(value.id, `${path}.id`);
  validateIdentifier(value.name, `${path}.name`);
  if (!typeFamilies.has(value.family as SqlTypeFamily))
    throw new Error(`${path}.family is invalid`);
  if (value.definition !== undefined)
    requireString(value.definition, `${path}.definition`);
}

export function parseCatalogSnapshot(value: unknown): CatalogSnapshot {
  if (!isRecord(value)) throw new Error("CatalogSnapshot must be an object");
  if (value.schemaVersion !== catalogSchemaVersion)
    throw new Error("Unsupported CatalogSnapshot schemaVersion");
  if (
    typeof value.id !== "string" ||
    typeof value.connectionId !== "string" ||
    typeof value.databaseId !== "string" ||
    typeof value.version !== "string" ||
    typeof value.generatedAt !== "string"
  )
    throw new Error("CatalogSnapshot identity fields are invalid");
  if (!dialects.has(value.dialect as string))
    throw new Error("CatalogSnapshot dialect is invalid");
  requireArray(value.databases, "databases", validateDatabase);
  requireArray(value.schemas, "schemas", validateSchema);
  requireArray(value.tables, "tables", validateTable);
  requireArray(value.views, "views", validateView);
  requireArray(value.materializedViews, "materializedViews", validateView);
  requireArray(value.functions, "functions", validateFunction);
  requireArray(value.procedures, "procedures", validateProcedure);
  requireArray(value.sequences, "sequences", validateSequence);
  requireArray(value.types, "types", validateTypeEntry);
  requireArray(value.triggers, "triggers", validateTrigger);
  requireArray(value.extensions, "extensions", validateExtension);
  requireArray(value.partitions, "partitions", validatePartition);
  requireArray(value.principals, "principals", validatePrincipal);
  requireArray(value.permissions, "permissions", validatePermission);
  requireArray(value.synonyms, "synonyms", validateSynonym);
  requireArray(value.foreignKeys, "foreignKeys", validateForeignKey);
  requireArray(value.indexes, "indexes", validateIndex);
  requireArray(value.constraints, "constraints", validateConstraint);
  return value as CatalogSnapshot;
}
