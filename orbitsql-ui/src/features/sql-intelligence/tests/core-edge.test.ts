import { describe, expect, it } from "vitest";
import type {
  CatalogFunction,
  CatalogSnapshot,
  CatalogType,
  CatalogView,
  ForeignKeyMetadata,
} from "../catalog";
import {
  buildCatalogIndex,
  createEmptyCatalogIndex,
  findObject,
  findObjectInExecutionContext,
  identifierKey,
  qualifiedNameKey,
} from "../catalog";
import {
  catalogSchemaVersion,
  parseCatalogSnapshot,
} from "../catalog/contract";
import {
  complete,
  isCompletionCurrent,
  tokenizeForCompletion,
} from "../completion/engine";
import { rankCompletionItems } from "../completion/ranking";
import {
  dialectRegistry,
  getDialect,
  isDialectKeyword,
  normalizeIdentifier,
  quoteIdentifier,
} from "../dialect/registry";
import { createSqlDocument } from "../document/factory";
import { cursorFromMarker } from "../document/test-utils";
import { isValidRange } from "../document/validation";
import {
  isWordToken,
  matchingParen,
  qualifiedNameFromTokens,
  tokenizeSql,
  tokenText,
} from "../parser/tokens";
import { buildCompletionContext } from "../semantic/context";
import {
  bindingColumn,
  bindingColumns,
  isCatalogTable,
} from "../semantic/types";
import { createCatalog, createSyntheticCatalogSnapshot } from "./fixtures";

function identifier(raw: string) {
  return { raw, normalized: raw, quoted: false, quoteStyle: "none" as const };
}

function request(
  markedSql: string,
  dialect: "postgresql" | "sqlserver" = "postgresql",
) {
  const marker = cursorFromMarker(markedSql);
  const document = createSqlDocument("edge", 3, marker.text, dialect);
  return { document, catalog: createCatalog(), position: marker.position };
}

describe("catalog and dialect contracts", () => {
  it("validates the versioned serialized snapshot boundary", () => {
    const snapshot = createCatalog().snapshot;
    const firstColumn = snapshot.tables[0].columns[0];
    const identifier = {
      raw: "contract",
      normalized: "contract",
      quoted: false,
      quoteStyle: "none" as const,
    };
    const parameter = {
      id: "parameter:contract",
      name: identifier,
      type: { family: "text" as const, name: "text" },
      direction: "in" as const,
      ordinal: 1,
    };
    const primaryKey = {
      id: "pk:contract",
      name: identifier,
      tableId: snapshot.tables[0].id,
      columnIds: [firstColumn.id],
    };
    const valid = {
      ...snapshot,
      dialect: "postgresql" as const,
      schemas: [
        {
          ...snapshot.schemas[0],
          comment: "schema",
        },
      ],
      tables: [
        {
          ...snapshot.tables[0],
          primaryKey,
          columns: [
            {
              ...firstColumn,
              defaultValue: "0",
              comment: "column",
              type: {
                family: "array" as const,
                name: "text[]",
                elementType: { family: "text" as const, name: "text" },
              },
            },
          ],
          uniqueConstraints: [primaryKey],
          checkConstraints: [
            {
              id: "check:contract",
              name: identifier,
              tableId: snapshot.tables[0].id,
              expression: "id > 0",
            },
          ],
          indexes: [
            {
              id: "index:contract",
              name: identifier,
              tableId: snapshot.tables[0].id,
              columnIds: [firstColumn.id],
              unique: true,
              primary: true,
            },
          ],
          estimatedRows: 1,
          comment: "table",
        },
      ],
      views: [
        {
          id: "view:contract",
          databaseId: "db:test",
          schemaId: "schema:public",
          name: identifier,
          columns: [firstColumn],
          comment: "view",
        },
      ],
      materializedViews: [],
      functions: [
        {
          id: "function:contract",
          name: identifier,
          parameters: [parameter],
          returnType: { family: "text" as const, name: "text" },
          variadic: true,
          overloads: ["contract(text)"],
          kind: "function" as const,
        },
      ],
      procedures: [
        {
          id: "procedure:contract",
          name: identifier,
          parameters: [parameter],
        },
      ],
      sequences: [
        {
          id: "sequence:contract",
          schemaId: "schema:public",
          name: identifier,
          type: { family: "integer" as const, name: "integer" },
        },
      ],
      types: [
        {
          id: "type:contract",
          name: identifier,
          family: "custom" as const,
          definition: "contract",
        },
      ],
      foreignKeys: [snapshot.foreignKeys[0]],
      indexes: [
        {
          id: "index:contract",
          name: identifier,
          tableId: snapshot.tables[0].id,
          columnIds: [firstColumn.id],
          unique: true,
          primary: true,
        },
      ],
      constraints: [primaryKey],
    };
    expect(catalogSchemaVersion).toBe(1);
    expect(parseCatalogSnapshot(snapshot)).toBe(snapshot);
    expect(parseCatalogSnapshot(valid)).toBe(valid);
    expect(() => parseCatalogSnapshot(null)).toThrow(/object/);
    expect(() =>
      parseCatalogSnapshot({ ...snapshot, schemaVersion: 2 }),
    ).toThrow(/schemaVersion/);
    expect(() =>
      parseCatalogSnapshot({ ...snapshot, tables: "invalid" }),
    ).toThrow(/tables must be an array/);
    expect(() =>
      parseCatalogSnapshot({ ...valid, dialect: "invalid" }),
    ).toThrow(/dialect/);
    expect(() => parseCatalogSnapshot({ ...valid, id: null })).toThrow(
      /identity/,
    );
    expect(() =>
      parseCatalogSnapshot({
        ...valid,
        schemas: [
          {
            ...valid.schemas[0],
            name: { ...identifier, quoteStyle: "invalid" },
          },
        ],
      }),
    ).toThrow(/quoteStyle/);
    expect(() =>
      parseCatalogSnapshot({
        ...valid,
        tables: [
          {
            ...valid.tables[0],
            columns: [
              {
                ...valid.tables[0].columns[0],
                type: { family: "invalid", name: "unknown" },
              },
            ],
          },
        ],
      }),
    ).toThrow(/family/);
    expect(() =>
      parseCatalogSnapshot({
        ...valid,
        functions: [
          {
            ...valid.functions[0],
            parameters: [{ ...parameter, direction: "invalid" }],
          },
        ],
      }),
    ).toThrow(/direction/);
    const invalid = (changes: Record<string, unknown>) =>
      expect(() => parseCatalogSnapshot({ ...valid, ...changes })).toThrow();
    invalid({ schemas: [null] });
    invalid({ databases: [null] });
    invalid({ tables: [null] });
    invalid({ views: [null] });
    invalid({ functions: [null] });
    invalid({ procedures: [null] });
    invalid({ sequences: [null] });
    invalid({ types: [{ ...valid.types[0], family: "invalid" }] });
    invalid({ foreignKeys: [null] });
    invalid({ indexes: [null] });
    invalid({ constraints: [null] });
    invalid({ triggers: [null] });
    invalid({ extensions: [null] });
    invalid({ partitions: [null] });
    invalid({ principals: [null] });
    invalid({ permissions: [null] });
    invalid({ synonyms: [null] });
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [{ ...valid.tables[0].columns[0], id: null }],
        },
      ],
    });
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [{ ...valid.tables[0].columns[0], name: null }],
        },
      ],
    });
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [
            {
              ...valid.tables[0].columns[0],
              type: { family: "text", name: "text", scale: "invalid" },
            },
          ],
        },
      ],
    });
    invalid({
      views: [
        {
          ...valid.views[0],
          comment: 1,
        },
      ],
    });
    invalid({
      procedures: [
        {
          ...valid.procedures[0],
          parameters: [null],
        },
      ],
    });
    invalid({
      types: [
        {
          ...valid.types[0],
          definition: 1,
        },
      ],
    });
    expect(
      parseCatalogSnapshot({
        ...valid,
        views: [{ ...valid.views[0], comment: undefined }],
        types: [{ ...valid.types[0], definition: undefined }],
      }),
    ).toBeDefined();
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [{ ...valid.tables[0].columns[0], ordinal: "invalid" }],
        },
      ],
    });
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [
            {
              ...valid.tables[0].columns[0],
              primaryKey: "invalid",
            },
          ],
        },
      ],
    });
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [
            {
              ...valid.tables[0].columns[0],
              foreignKeyMemberships: [1],
            },
          ],
        },
      ],
    });
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [
            {
              ...valid.tables[0].columns[0],
              type: null,
            },
          ],
        },
      ],
    });
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [
            {
              ...valid.tables[0].columns[0],
              type: { family: "text", name: "text", precision: "invalid" },
            },
          ],
        },
      ],
    });
    invalid({
      tables: [
        {
          ...valid.tables[0],
          columns: [
            {
              ...valid.tables[0].columns[0],
              type: { family: "text", name: "text", nullable: "invalid" },
            },
          ],
        },
      ],
    });
  });

  it("indexes schemas, qualified objects, relations, functions, and types", () => {
    const base = createCatalog().snapshot;
    const view: CatalogView = {
      id: "view:active",
      databaseId: "db:test",
      schemaId: "schema:public",
      name: {
        raw: "active_users",
        normalized: "active_users",
        quoted: false,
        quoteStyle: "none",
      },
      columns: [],
    };
    const fn: CatalogFunction = {
      id: "fn:now",
      name: {
        raw: "now",
        normalized: "now",
        quoted: false,
        quoteStyle: "none",
      },
      parameters: [],
      returnType: { family: "timestamp", name: "timestamp" },
      variadic: false,
      overloads: [],
    };
    const customType: CatalogType = {
      id: "type:status",
      name: {
        raw: "status",
        normalized: "status",
        quoted: false,
        quoteStyle: "none",
      },
      family: "enum",
    };
    const snapshot: CatalogSnapshot = {
      ...base,
      views: [view],
      materializedViews: [view],
      functions: [fn],
      types: [customType],
    };
    const index = buildCatalogIndex(snapshot);
    expect(index.schemasByName.get("public")?.id).toBe("schema:public");
    expect(findObject(index, "active_users", "public")?.id).toBe("view:active");
    expect(index.relationsByTableId.get("table:orders")?.length).toBe(1);
    expect(index.functionsByName.get("now")?.[0]?.id).toBe("fn:now");
    expect(index.typesByName.get("status")?.[0]?.id).toBe("type:status");
    expect(index.objectsByPrefix.get("act")?.[0]?.id).toBe("view:active");
    expect(index.functionsByPrefix.get("no")?.[0]?.id).toBe("fn:now");
    expect(index.typesByPrefix.get("sta")?.[0]?.id).toBe("type:status");
  });

  it("resolves objects through qualified and execution contexts", () => {
    const base = createCatalog().snapshot;
    const otherSchema = {
      id: "schema:other",
      catalogId: "other",
      databaseId: "db:other",
      name: {
        raw: "public",
        normalized: "public",
        quoted: false,
        quoteStyle: "none" as const,
      },
    };
    const otherUsers = {
      ...base.tables[0],
      id: "table:other:users",
      databaseId: "db:other",
      schemaId: otherSchema.id,
    };
    const index = buildCatalogIndex({
      ...base,
      databases: [
        { id: "db:test", name: identifier("test"), system: false },
        { id: "db:other", name: identifier("other"), system: false },
      ],
      schemas: [...base.schemas, otherSchema],
      tables: [...base.tables, otherUsers],
    });
    expect(
      findObject(index, {
        catalog: identifier("db:other"),
        schema: identifier("public"),
        object: identifier("users"),
      })?.id,
    ).toBe(otherUsers.id);
    expect(
      findObject(index, {
        schema: identifier("public"),
        object: identifier("users"),
      })?.id,
    ).toBe(otherUsers.id);
    expect(findObject(index, { object: identifier("users") })?.id).toBe(
      base.tables[0].id,
    );
    expect(findObject(index, "users", "public")?.id).toBe(otherUsers.id);
    expect(findObject(index, "missing")).toBeUndefined();
    expect(
      findObjectInExecutionContext(index, "users", {
        database: "other",
        schema: "public",
      })?.id,
    ).toBe(otherUsers.id);
    expect(
      findObjectInExecutionContext(index, "users", {
        catalog: "db:other",
        user: "public",
      })?.id,
    ).toBe(otherUsers.id);
    expect(findObjectInExecutionContext(index, "users")?.id).toBe(
      base.tables[0].id,
    );
    expect(
      findObjectInExecutionContext(index, "users", { schema: "missing" })?.id,
    ).toBe(base.tables[0].id);
  });

  it("handles identifiers and every dialect quoting style", () => {
    expect(identifierKey("Users")).toBe("users");
    expect(
      identifierKey({
        raw: "Users",
        normalized: "Users",
        quoted: true,
        quoteStyle: "double",
      }),
    ).toBe("users");
    expect(qualifiedNameKey(["public", undefined, "users"])).toBe(
      "public.users",
    );
    const qualifiedTokens = tokenizeSql(
      'catalog.test_db."User Table"',
      getDialect("postgresql"),
    );
    const qualified = qualifiedNameFromTokens(
      qualifiedTokens,
      0,
      qualifiedTokens.length - 1,
    );
    expect(qualified.name.catalog?.normalized).toBe("catalog");
    expect(qualified.name.schema?.normalized).toBe("test_db");
    expect(qualified.name.object.normalized).toBe("User Table");
    expect(qualified.name.object.quoted).toBe(true);
    expect(normalizeIdentifier("" + '"Name"', getDialect("postgresql"))).toBe(
      "Name",
    );
    expect(normalizeIdentifier("[Name]", getDialect("sqlserver"))).toBe("Name");
    expect(normalizeIdentifier("Name", getDialect("oracle"))).toBe("NAME");
    expect(normalizeIdentifier("Name", getDialect("generic"))).toBe("Name");
    expect(quoteIdentifier("a]b", getDialect("sqlserver"))).toBe("[a]]b]");
    expect(isDialectKeyword("select", getDialect("postgresql"))).toBe(true);
    expect(isDialectKeyword("custom_name", getDialect("postgresql"))).toBe(
      false,
    );
    expect(getDialect("unknown" as never).id).toBe("generic");
    expect(getDialect("postgresql").keywords.has("limit")).toBe(true);
    expect(getDialect("sqlserver").keywords.has("limit")).toBe(false);
    expect(getDialect("sqlserver").keywords.has("top")).toBe(true);
    expect(getDialect("postgresql").operators).toContain("@>");
    expect(getDialect("mysql").operators).not.toContain("::");
    const mutableRegistry = dialectRegistry as Map<string, unknown>;
    const genericDialect = mutableRegistry.get("generic");
    mutableRegistry.delete("generic");
    expect(() => getDialect("unknown" as never)).toThrow(/not registered/);
    mutableRegistry.set("generic", genericDialect);
    expect(createEmptyCatalogIndex("sqlite").snapshot.dialect).toBe("sqlite");
    expect(createSyntheticCatalogSnapshot("mysql").tables).toHaveLength(2);
    expect(quoteIdentifier("a`b", getDialect("mysql"))).toBe("`a``b`");
  });
});

describe("document and parser boundaries", () => {
  it("validates ranges and cursor markers", () => {
    expect(isValidRange({ from: 0, to: 3 }, 3)).toBe(true);
    expect(isValidRange({ from: 4, to: 3 }, 4)).toBe(false);
    expect(() => cursorFromMarker("SELECT")).toThrow();
    expect(() => cursorFromMarker("|SELECT|")).toThrow();
  });

  it("tokenizes comments, operators, numbers, punctuation, and incomplete quotes", () => {
    const tokens = tokenizeSql(
      "-- line\nSELECT 12.5, a::text, b >= 2 /* block */ FROM users",
      getDialect("postgresql"),
    );
    expect(tokens.some((token) => token.kind === "number")).toBe(true);
    expect(tokens.some((token) => token.text === "::")).toBe(true);
    expect(tokens.some((token) => token.text === ",")).toBe(true);
    expect(tokenText(undefined)).toBe("");
    expect(isWordToken(tokens.find((token) => token.text === "SELECT"))).toBe(
      true,
    );
    expect(isWordToken(tokens.find((token) => token.kind === "operator"))).toBe(
      false,
    );
    const incomplete = tokenizeSql(
      "SELECT 'unterminated",
      getDialect("postgresql"),
    );
    expect(incomplete[incomplete.length - 1]?.kind).toBe("string");
    expect(
      tokenizeSql('SELECT "a""b" FROM `users`', getDialect("mysql")).filter(
        (token) => token.kind === "quoted-identifier",
      ),
    ).toHaveLength(2);
    expect(matchingParen(tokenizeSql("(x)", getDialect("postgresql")), 0)).toBe(
      2,
    );
    expect(
      matchingParen(tokenizeSql("(x", getDialect("postgresql")), 0),
    ).toBeUndefined();
    expect(tokenizeSql("SELECT $1", getDialect("postgresql"))[1]?.kind).toBe(
      "parameter",
    );
    expect(tokenizeSql("SELECT ?", getDialect("mysql"))[1]?.kind).toBe(
      "parameter",
    );
    expect(tokenizeSql("SELECT @name", getDialect("sqlserver"))[1]?.kind).toBe(
      "parameter",
    );
    expect(tokenizeSql("SELECT :bind", getDialect("oracle"))[1]?.kind).toBe(
      "parameter",
    );
  });
});

describe("semantic context and completion branches", () => {
  it("covers keyword, column, unresolved qualifier, and table contexts", () => {
    expect(complete(request("|")).context.kind).toBe("statement-start");
    expect(complete(request("SELECT |")).context.kind).toBe(
      "select-projection",
    );
    expect(complete(request("SELECT CO|")).items[0]?.kind).toBe("function");
    expect(complete(request("SELECT missing.|")).items).toEqual([]);
    expect(complete(request("SELECT * FROM re|")).context.kind).toBe(
      "from-relation",
    );
    expect(
      complete(
        request("WITH recent AS (SELECT id FROM orders) SELECT * FROM re|"),
      ).items.some((item) => item.kind === "cte"),
    ).toBe(true);
    expect(
      buildCompletionContext(
        createSqlDocument("clamped", 1, "SELECT", "postgresql"),
        createCatalog(),
        100,
      ).range.to,
    ).toBe(6);
  });

  it("handles views, multiple statements, schema-qualified names, and missing joins", () => {
    const catalog = createCatalog();
    const marker = cursorFromMarker(
      "SELECT * FROM nope; SELECT u.| FROM public.users u",
    );
    const document = createSqlDocument("multi", 1, marker.text, "postgresql");
    const result = complete({ document, catalog, position: marker.position });
    expect(result.context.kind).toBe("qualified-column");
    expect(result.items).toHaveLength(3);
    expect(complete(request("SELECT * FROM active_|")).context.kind).toBe(
      "from-relation",
    );
  });

  it("exercises quoted aliases, explicit aliases, projections, and malformed scopes", () => {
    const catalog = createCatalog();
    const samples = [
      "SELECT o.id AS order_id FROM orders AS o WHERE |",
      "SELECT o.id, (o.total) AS amount FROM orders o WHERE |",
      "WITH recent (id) AS (SELECT id FROM orders), current AS (SELECT id FROM recent) SELECT * FROM current c WHERE |",
      "WITH broken (SELECT id FROM orders) SELECT |",
      "WITH broken AS (SELECT id FROM orders SELECT |",
      "SELECT * FROM (SELECT id FROM orders) WHERE |",
      "SELECT * FROM (SELECT id FROM orders) derived WHERE |",
      "SELECT * FROM . WHERE |",
      "SELECT (1) FROM orders WHERE |",
      "SELECT * FROM (SELECT |",
    ];
    for (const sample of samples) {
      const marker = cursorFromMarker(sample);
      const document = createSqlDocument(
        "malformed",
        1,
        marker.text,
        "postgresql",
      );
      expect(() =>
        complete({ document, catalog, position: marker.position }),
      ).not.toThrow();
    }
  });

  it("covers view completion and statement terminators", () => {
    const base = createCatalog().snapshot;
    const view: CatalogView = {
      id: "view:active",
      databaseId: "db:test",
      schemaId: "schema:public",
      name: {
        raw: "active_users",
        normalized: "active_users",
        quoted: false,
        quoteStyle: "none",
      },
      columns: [],
    };
    const catalog = buildCatalogIndex({ ...base, views: [view] });
    const marker = cursorFromMarker("SELECT * FROM active_|");
    const document = createSqlDocument("view", 1, marker.text, "postgresql");
    expect(
      complete({ document, catalog, position: marker.position }).items[0]?.kind,
    ).toBe("view");
    const first = cursorFromMarker("SELECT |;");
    const firstDocument = createSqlDocument(
      "first",
      1,
      first.text,
      "postgresql",
    );
    expect(
      complete({ document: firstDocument, catalog, position: first.position })
        .context.kind,
    ).toBe("select-projection");
  });

  it("supports rank filtering and semantic helper functions", () => {
    const result = complete(request("SELECT u.| FROM users u"));
    const binding = result.context.scope.bindings[0];
    expect(binding).toBeDefined();
    if (!binding) throw new Error("Expected a resolved binding");
    expect(bindingColumns(binding)).toHaveLength(3);
    expect(bindingColumn(binding, "id")?.name).toBe("id");
    expect(isCatalogTable(binding.object)).toBe(true);
    expect(
      rankCompletionItems(result.items, "u.i").map((item) => item.displayLabel),
    ).toEqual(["u.id"]);
    expect(tokenizeForCompletion(request("SELECT |"))).toHaveLength(1);
    expect(result.items[0]?.features?.semanticFit).toBe(1);
    expect(result.items[0]?.features?.prefixQuality).toBeGreaterThan(0);
  });

  it("completes catalog types and composite foreign-key predicates", () => {
    const base = createCatalog().snapshot;
    const composite: ForeignKeyMetadata = {
      id: "fk:composite",
      name: {
        raw: "orders_customer_composite",
        normalized: "orders_customer_composite",
        quoted: false,
        quoteStyle: "none",
      },
      sourceTableId: "table:orders",
      sourceColumnIds: ["column:orders:id", "column:orders:customer_id"],
      targetTableId: "table:customers",
      targetColumnIds: ["column:customers:id", "column:customers:id"],
    };
    const type = {
      id: "type:status",
      name: {
        raw: "status",
        normalized: "status",
        quoted: false,
        quoteStyle: "none" as const,
      },
      family: "enum" as const,
    };
    const catalog = buildCatalogIndex({
      ...base,
      types: [type],
      foreignKeys: [...base.foreignKeys, composite],
    });
    const typeMarker = cursorFromMarker(
      "SELECT * FROM users WHERE value AS st|",
    );
    expect(
      complete({
        document: createSqlDocument("type", 1, typeMarker.text, "postgresql"),
        catalog,
        position: typeMarker.position,
      }).context.kind,
    ).toBe("type");
    const joinMarker = cursorFromMarker(
      "SELECT * FROM orders o JOIN customers c ON |",
    );
    const join = complete({
      document: createSqlDocument(
        "composite",
        1,
        joinMarker.text,
        "postgresql",
      ),
      catalog,
      position: joinMarker.position,
    });
    expect(join.items.some((item) => item.insertText.includes(" AND "))).toBe(
      true,
    );
  });

  it("classifies an ON clause after a join and a WHERE clause as columns", () => {
    expect(
      complete(request("SELECT * FROM orders o JOIN customers c ON |")).context
        .kind,
    ).toBe("join-condition");
    expect(
      complete(
        request(
          "SELECT * FROM orders o JOIN customers c ON o.id = c.id WHERE |",
        ),
      ).context.kind,
    ).toBe("where-expression");
  });

  it("keeps alias and expected-type contexts isolated", () => {
    expect(complete(request("SELECT * FROM users AS |")).context.kind).toBe(
      "relation-alias",
    );
    expect(complete(request("SELECT * FROM users AS u|")).context.kind).toBe(
      "relation-alias",
    );
    expect(complete(request("SELECT id AS | FROM users")).context.kind).toBe(
      "select-alias",
    );
    expect(complete(request("SELECT id AS us| FROM users")).context.kind).toBe(
      "select-alias",
    );

    expect(
      complete(request("UPDATE users SET email = |")).context.expectedType?.type
        .family,
    ).toBe("text");
    expect(
      complete(request("INSERT INTO users (name) VALUES (|)")).context
        .expectedType?.type.family,
    ).toBe("text");
    expect(
      complete(request("SELECT * FROM users WHERE id > |")).context.expectedType
        ?.type.family,
    ).toBe("integer");
    expect(
      complete(request("SELECT UNKNOWN(|) FROM users")).context.expectedType,
    ).toBeUndefined();
  });

  it("keeps stale documents and catalogs from being accepted", () => {
    const response = complete(request("SELECT |"));
    expect(isCompletionCurrent(response, "edge", 3, "catalog-v1")).toBe(true);
    expect(isCompletionCurrent(response, "edge", 4, "catalog-v1")).toBe(false);
    expect(isCompletionCurrent(response, "edge", 3, "catalog-v2")).toBe(false);
  });

  it("uses editor syntax snapshots to suppress completion in protected nodes", () => {
    const marker = cursorFromMarker("SELECT 'abc|'");
    const response = complete({
      document: createSqlDocument("syntax", 1, marker.text, "postgresql"),
      catalog: createCatalog(),
      position: marker.position,
      syntax: {
        source: "codemirror",
        rootName: "Script",
        nodeName: "String",
        nodeFrom: 7,
        nodeTo: 12,
        errorTolerant: true,
      },
    });
    expect(response.context.kind).toBe("none");
    expect(response.items).toHaveLength(0);
  });

  it("tracks the active function argument through commas and nested calls", () => {
    const dateAdd = complete(
      request(
        "SELECT DATEADD('day', id) FROM users WHERE DATEADD('day', |",
        "sqlserver",
      ),
    );
    expect(dateAdd.context.kind).toBe("function-argument");
    expect(dateAdd.context.expectedType?.type.family).toBe("integer");
    expect(dateAdd.items.map((item) => item.displayLabel)).toContain("id");
    expect(dateAdd.items.map((item) => item.displayLabel)).not.toContain(
      "name",
    );
    expect(
      complete(request("SELECT COALESCE(SUM(|) FROM users")).context.kind,
    ).toBe("function-argument");
  });
});
