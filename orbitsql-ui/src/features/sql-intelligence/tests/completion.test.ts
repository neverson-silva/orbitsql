import { describe, expect, it } from "vitest";
import { buildCatalogIndex, type ColumnMetadata } from "../catalog";
import { complete } from "../completion/engine";
import { getDialect } from "../dialect/registry";
import { createSqlDocument } from "../document/factory";
import { cursorFromMarker } from "../document/test-utils";
import { createCatalog } from "./fixtures";

function run(markedSql: string, catalog = createCatalog()) {
  const marker = cursorFromMarker(markedSql);
  const document = createSqlDocument(
    "test-document",
    7,
    marker.text,
    "postgresql",
  );
  return complete({
    document,
    catalog,
    position: marker.position,
  });
}

describe("sql completion engine", () => {
  it("completes columns for an alias", () => {
    const result = run("SELECT u.| FROM users u");
    expect(result.context.kind).toBe("qualified-column");
    expect(result.items.map((item) => item.displayLabel)).toEqual([
      "u.email",
      "u.id",
      "u.name",
    ]);
    expect(result.items.every((item) => item.from === 9 && item.to === 9)).toBe(
      true,
    );
  });

  it("filters table names by the prefix after FROM", () => {
    const result = run("SELECT * FROM us|");
    expect(result.context.kind).toBe("from-relation");
    expect(result.items.map((item) => item.displayLabel)).toEqual(["users"]);
    expect(result.items[0]?.from).toBe(14);
    expect(result.items[0]?.to).toBe(16);
  });

  it("treats a completed wildcard as a select-list continuation", () => {
    const result = run("SELECT *|");
    expect(result.items.map((item) => item.displayLabel)).toContain("FROM");
    expect(result.items.map((item) => item.displayLabel)).not.toContain("ABS");
    expect(result.items.every((item) => item.kind === "keyword")).toBe(true);
  });

  it("keeps expression providers active at a select-item start", () => {
    expect(run("SELECT |").items.map((item) => item.displayLabel)).toContain(
      "ABS",
    );
    expect(run("SELECT *, |").items.map((item) => item.displayLabel)).toContain(
      "ABS",
    );
  });

  it("keeps function completion available in expression clauses", () => {
    for (const text of [
      "SELECT * FROM users WHERE |",
      "SELECT * FROM users HAVING |",
      "SELECT * FROM users GROUP BY |",
      "SELECT * FROM users ORDER BY |",
      "SELECT ABS(|",
    ]) {
      expect(run(text).items.map((item) => item.displayLabel)).toContain("ABS");
    }
  });

  it("filters a clause keyword prefix after a completed wildcard", () => {
    const result = run("SELECT * FR|");
    expect(result.items.map((item) => item.displayLabel)).toEqual(["FROM"]);
    expect(result.items.map((item) => item.displayLabel)).not.toContain(
      "FUNCTION",
    );
  });

  it("keeps wildcard continuation context across whitespace and comments", () => {
    for (const text of ["SELECT * |", "SELECT\n*\n|", "SELECT /* note */ *|"]) {
      const result = run(text);
      expect(result.items.map((item) => item.displayLabel)).toContain("FROM");
      expect(result.items.map((item) => item.displayLabel)).not.toContain(
        "ABS",
      );
    }
  });

  it("offers SQL Server schemas before unqualified relations", () => {
    const catalog = buildCatalogIndex({
      ...createCatalog().snapshot,
      dialect: "sqlserver",
    });
    const text = "SELECT * FROM ";
    const result = complete({
      document: createSqlDocument(
        "sqlserver-schema-first",
        1,
        text,
        "sqlserver",
      ),
      catalog,
      position: text.length,
    });
    expect(result.context.kind).toBe("from-relation");
    expect(result.items.map((item) => item.displayLabel)).toEqual(["public"]);
    expect(result.items.some((item) => item.kind === "table")).toBe(false);
  });

  it("offers schemas before relations for schema-aware dialects", () => {
    for (const dialect of ["postgresql", "mysql", "mariadb"] as const) {
      const catalog = buildCatalogIndex({
        ...createCatalog().snapshot,
        dialect,
      });
      const text = "SELECT * FROM ";
      const result = complete({
        document: createSqlDocument(
          `schema-first-${dialect}`,
          1,
          text,
          dialect,
        ),
        catalog,
        position: text.length,
      });
      expect(result.context.kind).toBe("from-relation");
      expect(result.items[0]).toMatchObject({
        displayLabel: "public",
        kind: "schema",
      });
    }
  });

  it("prefers a SQL Server schema over an ambiguous database name", () => {
    const base = createCatalog().snapshot;
    const database = {
      id: "database:trinus-uau",
      name: {
        raw: "trinus-uau",
        normalized: "trinus-uau",
        quoted: false,
        quoteStyle: "none" as const,
      },
      system: false,
    };
    const schema = {
      ...base.schemas[0],
      id: "schema:trinus-uau",
      name: {
        ...base.schemas[0].name,
        raw: "trinus-uau",
        normalized: "trinus-uau",
      },
    };
    const catalog = buildCatalogIndex({
      ...base,
      dialect: "sqlserver",
      databases: [database],
      schemas: [schema],
      tables: base.tables.map((table) => ({
        ...table,
        schemaId: schema.id,
      })),
    });
    const text = "SELECT * FROM [trinus-uau].";
    const result = complete({
      document: createSqlDocument(
        "sqlserver-ambiguous-schema",
        1,
        text,
        "sqlserver",
      ),
      catalog,
      position: text.length,
    });
    expect(result.items.map((item) => item.displayLabel)).toEqual([
      "customers",
      "orders",
      "users",
    ]);
    expect(result.items.every((item) => item.kind === "table")).toBe(true);
  });

  it("keeps SQL Server DML targets schema-first", () => {
    const catalog = buildCatalogIndex({
      ...createCatalog().snapshot,
      dialect: "sqlserver",
    });
    for (const text of ["INSERT INTO ", "UPDATE ", "DELETE FROM "]) {
      const result = complete({
        document: createSqlDocument(
          "sqlserver-dml-schema-first",
          1,
          text,
          "sqlserver",
        ),
        catalog,
        position: text.length,
      });
      expect(result.items.map((item) => item.displayLabel)).toEqual(["public"]);
      expect(result.items.every((item) => item.kind === "schema")).toBe(true);
    }
  });

  it("does not offer schemas while typing a relation alias", () => {
    const catalog = buildCatalogIndex({
      ...createCatalog().snapshot,
      dialect: "sqlserver",
    });
    const marker = cursorFromMarker("SELECT * FROM [trinus-uau].customer c|");
    const result = complete({
      document: createSqlDocument(
        "sqlserver-relation-alias",
        1,
        marker.text,
        "sqlserver",
      ),
      catalog,
      position: marker.position,
    });
    expect(result.context.kind).toBe("relation-alias");
    expect(result.items).toEqual([]);
  });

  it("offers columns for a relation alias in join conditions", () => {
    const marker = cursorFromMarker(
      "SELECT * FROM users u JOIN customers c ON c.|",
    );
    const result = complete({
      document: createSqlDocument(
        "join-alias-column",
        1,
        marker.text,
        "postgresql",
      ),
      catalog: createCatalog(),
      position: marker.position,
    });
    expect(result.context.kind).toBe("join-condition");
    expect(result.items.map((item) => item.displayLabel)).toEqual([
      "c.id",
      "c.name",
    ]);
  });

  it("keeps comma-separated relation aliases in scope", () => {
    const marker = cursorFromMarker(
      "SELECT * FROM users u, customers c WHERE c.|",
    );
    const result = complete({
      document: createSqlDocument(
        "comma-relation-alias",
        1,
        marker.text,
        "postgresql",
      ),
      catalog: createCatalog(),
      position: marker.position,
    });
    expect(result.items.map((item) => item.displayLabel)).toEqual([
      "c.id",
      "c.name",
    ]);
  });

  it("does not offer types while typing a SELECT alias", () => {
    const result = run("SELECT id AS us|");
    expect(result.context.kind).toBe("select-alias");
    expect(result.items).toEqual([]);
  });

  it("resolves catalog, schema, and object qualified relations", () => {
    const result = run("SELECT u.| FROM test_db.public.users u");
    expect(result.items.map((item) => item.displayLabel)).toEqual([
      "u.email",
      "u.id",
      "u.name",
    ]);
  });

  it("uses CTE output columns in the outer scope", () => {
    const result = run(
      "WITH recent AS (SELECT id, customer_id FROM orders) SELECT r.| FROM recent r",
    );
    expect(result.items.map((item) => item.displayLabel)).toEqual([
      "r.customer_id",
      "r.id",
    ]);
    const renamed = run(
      "WITH recent (order_id) AS (SELECT id FROM orders) SELECT r.| FROM recent r",
    );
    expect(renamed.items.map((item) => item.displayLabel)).toEqual([
      "r.order_id",
    ]);
  });

  it("uses derived table output columns", () => {
    const result = run("SELECT q.| FROM (SELECT id, total FROM orders) q");
    expect(result.items.map((item) => item.displayLabel)).toEqual([
      "q.id",
      "q.total",
    ]);
  });

  it("keeps correlated subquery completion in the inner scope", () => {
    const result = run(
      "SELECT * FROM customers c WHERE EXISTS (SELECT 1 FROM orders o WHERE o.customer_id = c.|)",
    );
    expect(result.items.map((item) => item.displayLabel)).toEqual([
      "c.id",
      "c.name",
    ]);
    const outer = run(
      "SELECT c.| FROM customers c WHERE EXISTS (SELECT 1 FROM orders o)",
    );
    expect(outer.items.map((item) => item.displayLabel)).not.toContain("o.id");
  });

  it("resolves clauses at the cursor query depth", () => {
    const innerJoin = run(
      "SELECT * FROM customers c WHERE EXISTS (SELECT * FROM orders o JOIN |)",
    );
    expect(innerJoin.context.kind).toBe("join-relation");
    expect(innerJoin.items.map((item) => item.displayLabel)).toContain(
      "customers",
    );
    expect(run("SELECT (|) FROM users").context.kind).toBe("select-projection");
  });

  it("suggests a foreign-key join predicate", () => {
    const result = run("SELECT * FROM orders o JOIN customers c ON |");
    expect(result.context.kind).toBe("join-condition");
    expect(result.items[0]?.insertText).toBe("o.customer_id = c.id");
  });

  it("marks inferred joins as heuristic when metadata has no foreign key", () => {
    const snapshot = createCatalog().snapshot;
    const catalog = buildCatalogIndex({ ...snapshot, foreignKeys: [] });
    const result = run("SELECT * FROM orders o JOIN customers c ON |", catalog);
    const candidate = result.items.find((item) =>
      item.insertText.includes("customer_id"),
    );
    expect(candidate?.provenance).toBe("unique-key");
    expect(candidate?.features?.relationshipConfidence).toBeLessThan(1);
  });

  it("generates every equality for a composite foreign key", () => {
    const base = createCatalog().snapshot;
    const orders = base.tables.find((table) => table.id === "table:orders");
    const customers = base.tables.find(
      (table) => table.id === "table:customers",
    );
    if (!orders || !customers) throw new Error("Expected relationship tables");
    const extraColumn = (tableId: string, id: string, name: string) =>
      ({
        id,
        tableId,
        ordinal: 3,
        name: {
          raw: name,
          normalized: name,
          quoted: false,
          quoteStyle: "none" as const,
        },
        type: { family: "integer" as const, name: "integer" },
        nullable: false,
        generated: false,
        identity: false,
        computed: false,
        primaryKey: false,
        foreignKeyMemberships: [],
        uniqueMemberships: [],
      }) satisfies ColumnMetadata;
    const source = extraColumn(
      orders.id,
      "column:orders:tenant_id",
      "tenant_id",
    );
    const target = extraColumn(
      customers.id,
      "column:customers:tenant_id",
      "tenant_id",
    );
    const catalog = buildCatalogIndex({
      ...base,
      tables: [
        { ...customers, columns: [...customers.columns, target] },
        { ...orders, columns: [...orders.columns, source] },
        ...base.tables.filter(
          (table) => table.id !== customers.id && table.id !== orders.id,
        ),
      ],
      foreignKeys: [
        {
          ...base.foreignKeys[0],
          sourceColumnIds: [
            "column:orders:customer_id",
            "column:orders:tenant_id",
          ],
          targetColumnIds: [
            "column:customers:id",
            "column:customers:tenant_id",
          ],
        },
      ],
    });
    const result = run("SELECT * FROM orders o JOIN customers c ON |", catalog);
    expect(result.items[0]?.insertText).toBe(
      "o.customer_id = c.id AND o.tenant_id = c.tenant_id",
    );
  });

  it("preserves document and catalog versions", () => {
    const result = run("SELECT u.| FROM users u");
    expect(result.documentId).toBe("test-document");
    expect(result.documentVersion).toBe(7);
    expect(result.catalogVersion).toBe("catalog-v1");
  });

  it("bounds results for an interactive completion lane", () => {
    const result = complete({
      document: createSqlDocument(
        "test-document",
        7,
        "SELECT * FROM |",
        "postgresql",
      ),
      catalog: createCatalog(),
      position: 14,
      maxItems: 2,
    });
    expect(result.items).toHaveLength(2);
    expect(result.isIncomplete).toBe(true);
  });

  it("stops before work when cancelled", () => {
    const controller = new AbortController();
    controller.abort();
    expect(() =>
      complete({
        document: createSqlDocument("test", 1, "SELECT |", "postgresql"),
        catalog: createCatalog(),
        position: 7,
        signal: controller.signal,
      }),
    ).toThrowError(/aborted/i);
  });

  it("cancels when the document changes during a synchronous pipeline", () => {
    let reads = 0;
    const signal = {
      get aborted() {
        reads += 1;
        return reads > 1;
      },
    } as unknown as AbortSignal;
    expect(() =>
      complete({
        document: createSqlDocument("test", 1, "SELECT ", "postgresql"),
        catalog: createCatalog(),
        position: 7,
        signal,
      }),
    ).toThrowError(/aborted/i);
  });

  it("completes insert columns and update set columns", () => {
    const insert = run("INSERT INTO users (na|");
    expect(insert.context.kind).toBe("insert-columns");
    expect(insert.items.map((item) => item.displayLabel)).toEqual(["name"]);
    const update = run("UPDATE users SET em|");
    expect(update.context.kind).toBe("update-set-column");
    expect(update.items.map((item) => item.displayLabel)).toEqual(["email"]);
  });

  it("infers comparison and function argument types", () => {
    const comparison = run("SELECT * FROM users u WHERE u.id > |");
    expect(comparison.context.expectedType?.type.family).toBe("integer");
    const functionArgument = run("SELECT SUM(|) FROM users u");
    expect(functionArgument.context.kind).toBe("function-argument");
    expect(functionArgument.items.map((item) => item.displayLabel)).toContain(
      "id",
    );
    expect(
      functionArgument.items.map((item) => item.displayLabel),
    ).not.toContain("name");
  });

  it("infers assignment and insert value types", () => {
    const update = run("UPDATE users SET name = |");
    expect(update.context.kind).toBe("update-set-value");
    expect(update.context.expectedType?.type.family).toBe("text");
    const insert = run("INSERT INTO users (name) VALUES (|");
    expect(insert.context.kind).toBe("insert-values");
    expect(insert.context.expectedType?.type.family).toBe("text");
    const secondValue = run("INSERT INTO users (name, email) VALUES ('old', |");
    expect(secondValue.context.expectedType?.type.family).toBe("text");
  });

  it("keeps qualified DML targets connected to their catalog object", () => {
    const update = run(
      "UPDATE test_db.public.users SET name = |",
      createCatalog(),
    );
    expect(update.context.expectedType?.type.family).toBe("text");
    const insert = run(
      "INSERT INTO test_db.public.users (na|",
      createCatalog(),
    );
    expect(insert.context.kind).toBe("insert-columns");
    expect(insert.context.prefix).toBe("na");
    expect(insert.items.map((item) => item.displayLabel)).toEqual(["name"]);
  });

  it("keeps DML completion and expected types scoped to the current statement", () => {
    const insert = run(
      "INSERT INTO users (email) VALUES ('old'); INSERT INTO users (na|",
    );
    expect(insert.items.map((item) => item.displayLabel)).toEqual(["name"]);
    const update = run("UPDATE users SET email = 'old'; UPDATE users SET na|");
    expect(update.items.map((item) => item.displayLabel)).toEqual(["name"]);
    const values = run(
      "INSERT INTO users (name) VALUES ('old'); INSERT INTO users (email) VALUES (|",
    );
    expect(values.context.expectedType?.type.family).toBe("text");
  });

  it("infers expected types through quoted identifiers", () => {
    const update = run('UPDATE "users" SET "name" = |');
    expect(update.context.expectedType?.type.family).toBe("text");
    const insert = run('INSERT INTO "users" ("name") VALUES (|');
    expect(insert.context.expectedType?.type.family).toBe("text");
    const comparison = run('SELECT * FROM "users" u WHERE u."id" > |');
    expect(comparison.context.expectedType?.type.family).toBe("integer");
  });

  it("offers deterministic values only for the expected boolean family", () => {
    const base = run("SELECT * FROM users WHERE id = |");
    expect(base.items.map((item) => item.displayLabel)).toContain("NULL");
    expect(base.items.map((item) => item.displayLabel)).not.toContain("TRUE");
    expect(
      run("SELECT * FROM users WHERE id | ").items.map(
        (item) => item.displayLabel,
      ),
    ).toContain("=");
    expect(
      run("SELECT * FROM users u JOIN customers c ON u.id | ").items.map(
        (item) => item.displayLabel,
      ),
    ).toContain("=");
  });

  it("offers catalog enum literals without querying live data", () => {
    const base = createCatalog().snapshot;
    const users = base.tables.find(
      (table) => table.name.normalized === "users",
    );
    if (!users) throw new Error("Expected users table");
    const enumUsers = {
      ...users,
      columns: [
        ...users.columns,
        {
          ...users.columns[0],
          id: "column:users:status",
          ordinal: 3,
          name: {
            raw: "status",
            normalized: "status",
            quoted: false,
            quoteStyle: "none" as const,
          },
          type: { family: "enum" as const, name: "enum('active','inactive')" },
          identity: false,
          primaryKey: false,
        },
      ],
    };
    const catalog = buildCatalogIndex({
      ...base,
      tables: [
        enumUsers,
        ...base.tables.filter((table) => table.id !== users.id),
      ],
      types: [
        {
          id: "type:status",
          schemaId: "schema:public",
          name: {
            raw: "status",
            normalized: "status",
            quoted: false,
            quoteStyle: "none" as const,
          },
          family: "enum",
          definition: "enum('active','inactive')",
        },
        {
          id: "type:description",
          schemaId: "schema:public",
          name: {
            raw: "description",
            normalized: "description",
            quoted: false,
            quoteStyle: "none" as const,
          },
          family: "text",
        },
      ],
    });
    const result = run("SELECT * FROM users WHERE status = |", catalog);
    expect(result.items.map((item) => item.displayLabel)).toEqual(
      expect.arrayContaining(["active", "inactive"]),
    );
    expect(
      result.items.find((item) => item.displayLabel === "active")?.insertText,
    ).toBe("'active'");
  });

  it("completes aliases in ORDER BY and qualified quoted identifiers", () => {
    const order = run("SELECT id AS user_id FROM users ORDER BY us|");
    expect(order.context.kind).toBe("order-by-expression");
    expect(order.items.some((item) => item.displayLabel === "user_id")).toBe(
      true,
    );
    const group = run("SELECT id AS user_id FROM users GROUP BY us|");
    expect(group.context.kind).toBe("group-by-expression");
    expect(group.items.some((item) => item.displayLabel === "user_id")).toBe(
      true,
    );
    const quoted = run('SELECT "u alias".na| FROM users AS "u alias"');
    expect(quoted.context).toMatchObject({
      qualifier: "u alias",
      prefix: "na",
      kind: "qualified-column",
    });
    expect(quoted.items.map((item) => item.displayLabel)).toEqual([
      "u alias.name",
    ]);
    expect(quoted.items[0]?.insertText).toBe("name");
    expect(quoted.items[0]?.from).toBe(17);
    expect(
      run("SELECT [u alias].na| FROM users AS [u alias]").items.map(
        (item) => item.displayLabel,
      ),
    ).toEqual(["u alias.name"]);
    expect(
      run("SELECT `u alias`.na| FROM users AS `u alias`").items.map(
        (item) => item.displayLabel,
      ),
    ).toEqual(["u alias.name"]);
  });

  it("inserts only the column after a qualified alias in clauses", () => {
    for (const text of [
      "SELECT * FROM users u WHERE u.|",
      "SELECT * FROM users u ORDER BY u.|",
    ]) {
      const result = run(text);
      expect(result.context.qualifier).toBe("u");
      expect(
        result.items.find((item) => item.displayLabel === "u.id")?.insertText,
      ).toBe("id");
    }
  });

  it("supports schema-qualified relation completion and every registry dialect", () => {
    const result = run("SELECT * FROM public.us|");
    expect(result.context.kind).toBe("from-relation");
    expect(result.items.map((item) => item.displayLabel)).toEqual(["users"]);
    const schema = run("CREATE SCHEMA pub|");
    expect(schema.context.kind).toBe("schema-name");
    expect(schema.items.map((item) => item.displayLabel)).toEqual(["public"]);
    for (const dialect of [
      "postgresql",
      "mysql",
      "mariadb",
      "sqlserver",
      "oracle",
      "sqlite",
    ] as const) {
      expect(
        complete({
          document: createSqlDocument(dialect, 1, "SELECT ", dialect),
          catalog: createCatalog(),
          position: 7,
        }).context.syntax,
      ).toMatchObject({ source: "raw" });
    }
  });

  it("completes relations after a bracket-quoted schema", () => {
    const catalog = createCatalog();
    const schema = {
      ...catalog.snapshot.schemas[0],
      id: "schema:trinus-uau",
      name: {
        ...catalog.snapshot.schemas[0].name,
        raw: "trinus-uau",
        normalized: "trinus-uau",
      },
    };
    const scopedCatalog = buildCatalogIndex({
      ...catalog.snapshot,
      schemas: [schema],
      tables: catalog.snapshot.tables.map((table) => ({
        ...table,
        schemaId: schema.id,
      })),
    });
    const result = run("SELECT * FROM [trinus-uau].|", scopedCatalog);
    expect(result.context).toMatchObject({
      kind: "from-relation",
      qualifier: "trinus-uau",
      prefix: "",
    });
    expect(result.items.map((item) => item.displayLabel)).toEqual(
      expect.arrayContaining(["users", "customers", "orders"]),
    );
  });

  it("keeps semantic completion consistent across the active provider dialects", () => {
    for (const dialect of ["postgresql", "mysql", "mariadb"] as const) {
      const marker = cursorFromMarker("SELECT u.na| FROM users u");
      const result = complete({
        document: createSqlDocument(
          `dialect-${dialect}`,
          1,
          marker.text,
          dialect,
        ),
        catalog: createCatalog(),
        position: marker.position,
      });
      expect(result.context.kind).toBe("qualified-column");
      expect(result.items.map((item) => item.displayLabel)).toEqual(["u.name"]);
    }
  });

  it("uses dialect-specific function signatures", () => {
    const dateAdd = getDialect("sqlserver").functions.get("DATEADD");
    expect(dateAdd).toEqual({
      name: "DATEADD",
      returnFamily: "timestamp",
      argumentFamilies: ["text", "integer", "timestamp"],
    });
    expect(getDialect("postgresql").functions.has("DATEADD")).toBe(false);
  });

  it("offers a deterministic statement snippet only for a matching prefix", () => {
    const result = run("sel|");
    expect(result.items.some((item) => item.kind === "snippet")).toBe(true);
    expect(
      result.items.find((item) => item.kind === "snippet")?.snippet,
    ).toContain("SELECT");
    expect(run("x|").items.some((item) => item.kind === "snippet")).toBe(false);
  });
});
