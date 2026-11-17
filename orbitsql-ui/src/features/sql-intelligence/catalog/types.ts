import type { DialectId } from "../dialect/types";

export type SqlTypeFamily =
  | "unknown"
  | "null"
  | "boolean"
  | "integer"
  | "decimal"
  | "float"
  | "text"
  | "binary"
  | "date"
  | "time"
  | "timestamp"
  | "interval"
  | "uuid"
  | "json"
  | "xml"
  | "array"
  | "enum"
  | "spatial"
  | "custom";

export type SqlTypeRef = Readonly<{
  family: SqlTypeFamily;
  name: string;
  nullable?: boolean;
  precision?: number;
  scale?: number;
  elementType?: SqlTypeRef;
}>;

export type IdentifierQuoteStyle = "none" | "double" | "backtick" | "bracket";

export type Identifier = Readonly<{
  raw: string;
  normalized: string;
  quoted: boolean;
  quoteStyle: IdentifierQuoteStyle;
}>;

export type QualifiedName = Readonly<{
  catalog?: Identifier;
  schema?: Identifier;
  object: Identifier;
}>;

export type CatalogSchema = Readonly<{
  id: string;
  catalogId?: string;
  databaseId: string;
  name: Identifier;
  comment?: string;
}>;

export type CatalogDatabase = Readonly<{
  id: string;
  name: Identifier;
  system: boolean;
}>;

export type PrimaryKeyMetadata = Readonly<{
  id: string;
  name: Identifier;
  tableId: string;
  columnIds: readonly string[];
}>;

export type UniqueConstraintMetadata = Readonly<{
  id: string;
  name: Identifier;
  tableId: string;
  columnIds: readonly string[];
}>;

export type CheckConstraintMetadata = Readonly<{
  id: string;
  name: Identifier;
  tableId: string;
  expression?: string;
}>;

export type IndexMetadata = Readonly<{
  id: string;
  name: Identifier;
  tableId: string;
  columnIds: readonly string[];
  unique: boolean;
  primary: boolean;
}>;

export type ForeignKeyMetadata = Readonly<{
  id: string;
  name: Identifier;
  sourceTableId: string;
  sourceColumnIds: readonly string[];
  targetTableId: string;
  targetColumnIds: readonly string[];
}>;

export type ColumnMetadata = Readonly<{
  id: string;
  tableId: string;
  ordinal: number;
  name: Identifier;
  type: SqlTypeRef;
  nullable: boolean;
  generated: boolean;
  identity: boolean;
  computed: boolean;
  defaultValue?: string;
  primaryKey: boolean;
  foreignKeyMemberships: readonly string[];
  uniqueMemberships: readonly string[];
  comment?: string;
}>;

export type CatalogTable = Readonly<{
  id: string;
  catalogId?: string;
  databaseId: string;
  schemaId: string;
  name: Identifier;
  columns: readonly ColumnMetadata[];
  primaryKey?: PrimaryKeyMetadata;
  foreignKeys: readonly ForeignKeyMetadata[];
  uniqueConstraints: readonly UniqueConstraintMetadata[];
  checkConstraints: readonly CheckConstraintMetadata[];
  indexes: readonly IndexMetadata[];
  estimatedRows?: number;
  comment?: string;
  temporary: boolean;
  foreign?: boolean;
}>;

export type CatalogView = Readonly<{
  id: string;
  catalogId?: string;
  databaseId: string;
  schemaId: string;
  name: Identifier;
  columns: readonly ColumnMetadata[];
  comment?: string;
}>;

export type CatalogFunction = Readonly<{
  id: string;
  schemaId?: string;
  name: Identifier;
  parameters: readonly CatalogParameter[];
  returnType: SqlTypeRef;
  variadic: boolean;
  overloads: readonly string[];
  kind?: "function" | "aggregate" | "builtin";
}>;

export type CatalogProcedure = Readonly<{
  id: string;
  schemaId?: string;
  name: Identifier;
  parameters: readonly CatalogParameter[];
}>;

export type CatalogParameter = Readonly<{
  id: string;
  name: Identifier;
  type: SqlTypeRef;
  direction: "in" | "out" | "inout";
  ordinal: number;
}>;

export type CatalogSequence = Readonly<{
  id: string;
  schemaId: string;
  name: Identifier;
  type: SqlTypeRef;
}>;

export type CatalogType = Readonly<{
  id: string;
  schemaId?: string;
  name: Identifier;
  family: SqlTypeFamily;
  definition?: string;
}>;

export type CatalogTrigger = Readonly<{
  id: string;
  schemaId?: string;
  tableId?: string;
  name: Identifier;
  enabled: boolean;
  definition?: string;
}>;

export type CatalogExtension = Readonly<{
  id: string;
  name: Identifier;
  version?: string;
}>;

export type CatalogPartition = Readonly<{
  id: string;
  tableId: string;
  name?: Identifier;
  parentId?: string;
  bound?: string;
}>;

export type CatalogPrincipal = Readonly<{
  id: string;
  name: Identifier;
  kind: string;
  login?: string;
}>;

export type CatalogPermission = Readonly<{
  id: string;
  principalId: string;
  objectId?: string;
  permission: string;
  state: string;
}>;

export type CatalogSynonym = Readonly<{
  id: string;
  schemaId: string;
  name: Identifier;
  target: string;
}>;

export type CatalogSnapshot = Readonly<{
  schemaVersion: number;
  id: string;
  connectionId: string;
  databaseId: string;
  dialect: DialectId;
  version: string;
  generatedAt: string;
  databases: readonly CatalogDatabase[];
  schemas: readonly CatalogSchema[];
  tables: readonly CatalogTable[];
  views: readonly CatalogView[];
  materializedViews: readonly CatalogView[];
  functions: readonly CatalogFunction[];
  procedures: readonly CatalogProcedure[];
  sequences: readonly CatalogSequence[];
  types: readonly CatalogType[];
  triggers: readonly CatalogTrigger[];
  extensions: readonly CatalogExtension[];
  partitions: readonly CatalogPartition[];
  principals: readonly CatalogPrincipal[];
  permissions: readonly CatalogPermission[];
  synonyms: readonly CatalogSynonym[];
  foreignKeys: readonly ForeignKeyMetadata[];
  indexes: readonly IndexMetadata[];
  constraints: readonly (
    | PrimaryKeyMetadata
    | UniqueConstraintMetadata
    | CheckConstraintMetadata
  )[];
}>;

export type CatalogObject = CatalogTable | CatalogView;

export type CatalogIndex = Readonly<{
  snapshot: CatalogSnapshot;
  schemasById: ReadonlyMap<string, CatalogSchema>;
  schemasByName: ReadonlyMap<string, CatalogSchema>;
  objectsByQualifiedName: ReadonlyMap<string, CatalogObject>;
  objectsByUnqualifiedName: ReadonlyMap<string, readonly CatalogObject[]>;
  objectsByPrefix: ReadonlyMap<string, readonly CatalogObject[]>;
  columnsByTableId: ReadonlyMap<string, readonly ColumnMetadata[]>;
  relationsByTableId: ReadonlyMap<string, readonly ForeignKeyMetadata[]>;
  functionsByName: ReadonlyMap<string, readonly CatalogFunction[]>;
  functionsByPrefix: ReadonlyMap<string, readonly CatalogFunction[]>;
  typesByName: ReadonlyMap<string, readonly CatalogType[]>;
  typesByPrefix: ReadonlyMap<string, readonly CatalogType[]>;
}>;

export type ReadonlyMap<K, V> = Readonly<Map<K, V>>;
