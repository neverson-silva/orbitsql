import {
  buildCatalogIndex,
  type CatalogTable,
  type CatalogSnapshot,
  type ColumnMetadata,
  type ForeignKeyMetadata,
  type Identifier,
  type SqlTypeRef,
} from "../catalog";

function identifier(name: string): Identifier {
  return { raw: name, normalized: name, quoted: false, quoteStyle: "none" };
}

function type(name: string, family: SqlTypeRef["family"] = "text"): SqlTypeRef {
  return { family, name };
}

function column(
  tableId: string,
  id: string,
  name: string,
  ordinal: number,
  valueType: SqlTypeRef = type("text"),
): ColumnMetadata {
  return {
    id,
    tableId,
    ordinal,
    name: identifier(name),
    type: valueType,
    nullable: false,
    generated: false,
    identity: name === "id",
    computed: false,
    primaryKey: name === "id",
    foreignKeyMemberships: [],
    uniqueMemberships: [],
  };
}

const users = {
  id: "table:users",
  catalogId: "test_db",
  databaseId: "db:test",
  schemaId: "schema:public",
  name: identifier("users"),
  columns: [
    column(
      "table:users",
      "column:users:id",
      "id",
      0,
      type("integer", "integer"),
    ),
    column("table:users", "column:users:name", "name", 1),
    column("table:users", "column:users:email", "email", 2),
  ],
  foreignKeys: [],
  uniqueConstraints: [],
  checkConstraints: [],
  indexes: [],
  temporary: false,
};

const customers = {
  id: "table:customers",
  catalogId: "test_db",
  databaseId: "db:test",
  schemaId: "schema:public",
  name: identifier("customers"),
  columns: [
    column(
      "table:customers",
      "column:customers:id",
      "id",
      0,
      type("integer", "integer"),
    ),
    column("table:customers", "column:customers:name", "name", 1),
  ],
  foreignKeys: [],
  uniqueConstraints: [],
  checkConstraints: [],
  indexes: [],
  temporary: false,
};

const orders = {
  id: "table:orders",
  catalogId: "test_db",
  databaseId: "db:test",
  schemaId: "schema:public",
  name: identifier("orders"),
  columns: [
    column(
      "table:orders",
      "column:orders:id",
      "id",
      0,
      type("integer", "integer"),
    ),
    column(
      "table:orders",
      "column:orders:customer_id",
      "customer_id",
      1,
      type("integer", "integer"),
    ),
    column(
      "table:orders",
      "column:orders:total",
      "total",
      2,
      type("numeric", "decimal"),
    ),
  ],
  foreignKeys: [],
  uniqueConstraints: [],
  checkConstraints: [],
  indexes: [],
  temporary: false,
};

const orderCustomer: ForeignKeyMetadata = {
  id: "fk:orders:customers",
  name: identifier("orders_customer_id_fkey"),
  sourceTableId: orders.id,
  sourceColumnIds: ["column:orders:customer_id"],
  targetTableId: customers.id,
  targetColumnIds: ["column:customers:id"],
};

export function createCatalog() {
  const snapshot: CatalogSnapshot = {
    schemaVersion: 1,
    id: "snapshot:test",
    connectionId: "connection:test",
    databaseId: "db:test",
    dialect: "postgresql",
    version: "catalog-v1",
    generatedAt: "2026-01-01T00:00:00.000Z",
    databases: [],
    schemas: [
      {
        id: "schema:public",
        catalogId: "test_db",
        databaseId: "db:test",
        name: identifier("public"),
      },
    ],
    tables: [users, customers, orders],
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
    foreignKeys: [orderCustomer],
    indexes: [],
    constraints: [],
  };
  return buildCatalogIndex(snapshot);
}

function syntheticIdentifier(value: string): Identifier {
  return { raw: value, normalized: value, quoted: false, quoteStyle: "none" };
}

function syntheticType(
  name: string,
  family: SqlTypeRef["family"],
): SqlTypeRef {
  return { family, name };
}

function syntheticColumn(
  tableId: string,
  id: string,
  name: string,
  ordinal: number,
  valueType: SqlTypeRef,
  identity = false,
): ColumnMetadata {
  return {
    id,
    tableId,
    ordinal,
    name: syntheticIdentifier(name),
    type: valueType,
    nullable: false,
    generated: false,
    identity,
    computed: false,
    primaryKey: identity,
    foreignKeyMemberships: [],
    uniqueMemberships: [],
  };
}

function syntheticTable(
  id: string,
  name: string,
  columns: readonly ColumnMetadata[],
): CatalogTable {
  return {
    id,
    catalogId: "synthetic",
    databaseId: "synthetic",
    schemaId: "schema:public",
    name: syntheticIdentifier(name),
    columns,
    foreignKeys: [],
    uniqueConstraints: [],
    checkConstraints: [],
    indexes: [],
    temporary: false,
  };
}

export function createSyntheticCatalogSnapshot(
  dialect: CatalogSnapshot["dialect"] = "postgresql",
): CatalogSnapshot {
  const usersId = "table:synthetic:users";
  const ordersId = "table:synthetic:orders";
  const users = syntheticTable(usersId, "users", [
    syntheticColumn(
      usersId,
      "column:synthetic:users:id",
      "id",
      0,
      syntheticType("integer", "integer"),
      true,
    ),
    syntheticColumn(
      usersId,
      "column:synthetic:users:name",
      "name",
      1,
      syntheticType("text", "text"),
    ),
    syntheticColumn(
      usersId,
      "column:synthetic:users:email",
      "email",
      2,
      syntheticType("text", "text"),
    ),
  ]);
  const orders = syntheticTable(ordersId, "orders", [
    syntheticColumn(
      ordersId,
      "column:synthetic:orders:id",
      "id",
      0,
      syntheticType("integer", "integer"),
      true,
    ),
    syntheticColumn(
      ordersId,
      "column:synthetic:orders:customer_id",
      "customer_id",
      1,
      syntheticType("integer", "integer"),
    ),
    syntheticColumn(
      ordersId,
      "column:synthetic:orders:total",
      "total",
      2,
      syntheticType("numeric", "decimal"),
    ),
  ]);
  const relation: ForeignKeyMetadata = {
    id: "foreign-key:synthetic:orders:users",
    name: syntheticIdentifier("orders_customer_id_fkey"),
    sourceTableId: ordersId,
    sourceColumnIds: ["column:synthetic:orders:customer_id"],
    targetTableId: usersId,
    targetColumnIds: ["column:synthetic:users:id"],
  };
  const tables = [
    {
      ...users,
      columns: users.columns.map((value) =>
        value.name.normalized === "id"
          ? { ...value, uniqueMemberships: ["index:synthetic:users:pk"] }
          : value,
      ),
    },
    {
      ...orders,
      columns: orders.columns.map((value) =>
        value.name.normalized === "customer_id"
          ? { ...value, foreignKeyMemberships: [relation.id] }
          : value,
      ),
      foreignKeys: [relation],
    },
  ];
  return {
    schemaVersion: 1,
    id: `catalog:synthetic:${dialect}`,
    connectionId: "connection:synthetic",
    databaseId: "synthetic",
    dialect,
    version: "synthetic-v1",
    generatedAt: "1970-01-01T00:00:00.000Z",
    databases: [],
    schemas: [
      {
        id: "schema:public",
        catalogId: "synthetic",
        databaseId: "synthetic",
        name: syntheticIdentifier("public"),
      },
    ],
    tables,
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
    foreignKeys: [relation],
    indexes: [],
    constraints: [],
  };
}
