import { describe, expect, it } from "vitest";
import { buildCatalogIndex } from "../catalog";
import { complete } from "../completion/engine";
import {
  provideAliasCandidates,
  provideColumnCandidates,
  provideFunctionCandidates,
  provideInsertColumnCandidates,
  provideKeywordCandidates,
  provideOperatorCandidates,
  provideRelationCandidates,
  provideSchemaCandidates,
  provideTypeCandidates,
  provideUpdateColumnCandidates,
  provideValueCandidates,
} from "../completion/providers";
import { evaluateRankingQuality } from "../completion/quality";
import { rankCompletionItems } from "../completion/ranking";
import { provideSnippetCandidates } from "../completion/snippets";
import type { CompletionContext, CompletionItem } from "../completion/types";
import { getDialect } from "../dialect/registry";
import { createSqlDocument } from "../document/factory";
import { cursorFromMarker } from "../document/test-utils";
import { buildRelationshipGraph } from "../semantic/relations";
import { createCatalog } from "./fixtures";

function identifier(raw: string) {
  return { raw, normalized: raw, quoted: false, quoteStyle: "none" as const };
}

function request(
  markedSql: string,
  catalog = createCatalog(),
  dialect: "postgresql" | "sqlserver" = "postgresql",
) {
  const marker = cursorFromMarker(markedSql);
  const document = createSqlDocument("coverage", 1, marker.text, dialect);
  return { document, catalog, position: marker.position };
}

function context(
  markedSql: string,
  catalog = createCatalog(),
  dialect: "postgresql" | "sqlserver" = "postgresql",
): {
  context: CompletionContext;
  request: ReturnType<typeof request>;
} {
  const current = request(markedSql, catalog, dialect);
  return { context: complete(current).context, request: current };
}

describe("completion providers and ranking", () => {
  it("covers catalog provider families and hard filtering", () => {
    const catalog = createCatalog();
    const relation = context("SELECT * FROM us|");
    const schema = context("SELECT * FROM public|");
    const select = context("SELECT id AS selected_id FROM users ORDER BY se|");
    const type = buildCatalogIndex({
      ...catalog.snapshot,
      types: [
        {
          id: "type:status",
          schemaId: "schema:public",
          name: {
            raw: "status",
            normalized: "status",
            quoted: false,
            quoteStyle: "none",
          },
          family: "enum",
        },
      ],
    });

    expect(
      provideRelationCandidates(relation.request, relation.context),
    ).toHaveLength(1);
    expect(
      provideSchemaCandidates(schema.request, {
        ...schema.context,
        kind: "schema-name",
        prefix: "pub",
      }),
    ).toHaveLength(1);
    expect(
      provideFunctionCandidates(select.request, {
        ...select.context,
        prefix: "co",
      })[0]?.kind,
    ).toBe("function");
    const catalogFunction = buildCatalogIndex({
      ...catalog.snapshot,
      functions: [
        {
          id: "fn:current_user",
          name: {
            raw: "current_user",
            normalized: "current_user",
            quoted: false,
            quoteStyle: "none",
          },
          parameters: [],
          returnType: { family: "text", name: "text" },
          variadic: false,
          overloads: [],
        },
        {
          id: "fn:count",
          name: {
            raw: "count",
            normalized: "count",
            quoted: false,
            quoteStyle: "none",
          },
          parameters: [],
          returnType: { family: "text", name: "text" },
          variadic: false,
          overloads: [],
        },
      ],
    });
    expect(
      provideFunctionCandidates(
        { ...select.request, catalog: catalogFunction },
        { ...select.context, prefix: "current" },
      )[0]?.sourceId,
    ).toBe("fn:current_user");
    expect(
      provideFunctionCandidates(
        { ...select.request, catalog: catalogFunction },
        { ...select.context, prefix: "cou" },
      ).map((item) => item.displayLabel),
    ).toEqual(["count"]);
    expect(
      provideTypeCandidates(
        { ...select.request, catalog: type },
        { ...select.context, kind: "type", prefix: "sta" },
      )[0]?.displayLabel,
    ).toBe("status");
    expect(
      provideKeywordCandidates(select.request, {
        ...select.context,
        kind: "keyword",
        prefix: "",
      }).length,
    ).toBeGreaterThan(10);
    expect(
      provideAliasCandidates(select.request, select.context).some(
        (item) => item.displayLabel === "selected_id",
      ),
    ).toBe(true);
    const numeric = context("SELECT SUM(|) FROM orders");
    expect(
      provideColumnCandidates(numeric.request, numeric.context).some(
        (item) => item.displayLabel === "total",
      ),
    ).toBe(true);
    expect(
      provideColumnCandidates(numeric.request, {
        ...numeric.context,
        kind: "function-argument",
        expectedType: undefined,
      }),
    ).not.toHaveLength(0);

    const secondSchema = {
      id: "schema:private",
      databaseId: "db:test",
      name: {
        raw: "private",
        normalized: "private",
        quoted: false,
        quoteStyle: "none" as const,
      },
    };
    const privateTable = {
      ...catalog.snapshot.tables[0],
      id: "table:private_users",
      schemaId: secondSchema.id,
      name: {
        raw: "private_users",
        normalized: "private_users",
        quoted: false,
        quoteStyle: "none" as const,
      },
    };
    const multiSchema = buildCatalogIndex({
      ...catalog.snapshot,
      schemas: [...catalog.snapshot.schemas, secondSchema],
      tables: [...catalog.snapshot.tables, privateTable],
    });
    expect(
      provideRelationCandidates(
        { ...relation.request, catalog: multiSchema },
        { ...relation.context, prefix: "", qualifier: "public" },
      ),
    ).toHaveLength(3);
  });

  it("covers insert, update, value, and unresolved source paths", () => {
    const insert = context("INSERT INTO users (na|");
    const update = context("UPDATE users SET em|");
    const values = context("INSERT INTO users (name) VALUES (|");
    expect(
      provideInsertColumnCandidates(insert.request, insert.context).map(
        (item) => item.displayLabel,
      ),
    ).toEqual(["name", "email"]);
    expect(
      provideInsertColumnCandidates(
        context("INSERT INTO users (name, em|").request,
        context("INSERT INTO users (name, em|").context,
      ).map((item) => item.displayLabel),
    ).toEqual(["email"]);
    expect(
      provideUpdateColumnCandidates(update.request, update.context).map(
        (item) => item.displayLabel,
      ),
    ).toEqual(["name", "email"]);
    expect(
      provideUpdateColumnCandidates(
        context("UPDATE users SET name = 1, em|").request,
        context("UPDATE users SET name = 1, em|").context,
      ).map((item) => item.displayLabel),
    ).toEqual(["email"]);
    expect(
      provideValueCandidates(values.request, values.context).map(
        (item) => item.displayLabel,
      ),
    ).toContain("NULL");
    const operator = context("SELECT * FROM users WHERE id |");
    expect(operator.context.operatorExpected).toBe(true);
    expect(
      provideOperatorCandidates(operator.request, operator.context).map(
        (item) => item.displayLabel,
      ),
    ).toContain("=");
    expect(
      provideOperatorCandidates(operator.request, {
        ...operator.context,
        prefix: "zz",
      }),
    ).toEqual([]);
    expect(
      provideInsertColumnCandidates(
        request("INSERT INTO missing (|"),
        insert.context,
      ),
    ).toEqual([]);
    expect(
      provideUpdateColumnCandidates(
        request("UPDATE missing SET x = |"),
        update.context,
      ),
    ).toEqual([]);
    expect(
      provideInsertColumnCandidates(request("INSERT INTO|"), insert.context),
    ).toEqual([]);
    expect(
      provideInsertColumnCandidates(
        request("INSERT INTO users|"),
        insert.context,
      ).map((item) => item.displayLabel),
    ).toEqual(["name", "email"]);
    expect(
      provideUpdateColumnCandidates(
        request("UPDATE users|"),
        update.context,
      ).map((item) => item.displayLabel),
    ).toEqual(["name", "email"]);
    const qualifiedInsert = context("INSERT INTO public.users (na|");
    expect(
      provideInsertColumnCandidates(
        qualifiedInsert.request,
        qualifiedInsert.context,
      ).map((item) => item.displayLabel),
    ).toEqual(["name", "email"]);
    const qualifiedUpdate = context("UPDATE public.users SET em|");
    expect(
      provideUpdateColumnCandidates(
        qualifiedUpdate.request,
        qualifiedUpdate.context,
      ).map((item) => item.displayLabel),
    ).toEqual(["name", "email"]);
    expect(
      complete(request("INSERT INTO users (id) VALUES (|")).context
        .expectedType,
    ).toBeUndefined();
  });

  it("returns boolean constants when the expected type is boolean", () => {
    const base = createCatalog().snapshot;
    const users = base.tables[0];
    if (!users) throw new Error("Expected users table");
    const boolUsers = {
      ...users,
      columns: users.columns.map((column) =>
        column.name.normalized === "name"
          ? { ...column, type: { family: "boolean" as const, name: "boolean" } }
          : column,
      ),
    };
    const catalog = buildCatalogIndex({
      ...base,
      tables: [boolUsers, ...base.tables.slice(1)],
    });
    const current = request("SELECT * FROM users WHERE name = |", catalog);
    const result = complete(current);
    expect(result.context.expectedType?.type.family).toBe("boolean");
    expect(result.items.map((item) => item.displayLabel)).toContain("TRUE");
    expect(result.items.map((item) => item.displayLabel)).toContain("FALSE");
  });

  it("covers relationship graph fallbacks and missing relation columns", () => {
    const base = createCatalog().snapshot;
    const withoutRelations = buildCatalogIndex({ ...base, foreignKeys: [] });
    const current = request(
      "SELECT * FROM orders o JOIN customers c ON |",
      withoutRelations,
    );
    const result = complete(current);
    expect(result.items.some((item) => item.provenance === "unique-key")).toBe(
      true,
    );
    const missingRelation = {
      ...base.foreignKeys[0],
      sourceColumnIds: ["column:missing"],
    };
    const broken = buildCatalogIndex({
      ...base,
      foreignKeys: [missingRelation],
    });
    const graph = buildRelationshipGraph(broken);
    expect(graph.byTableId.get("table:orders")).toHaveLength(1);
    expect(
      complete(request("SELECT * FROM orders o JOIN customers c ON |", broken))
        .items,
    ).toEqual([]);

    const withoutKeys = base.tables.map((table) =>
      table.id === "table:customers"
        ? {
            ...table,
            columns: table.columns.map((column) => ({
              ...column,
              primaryKey: false,
              uniqueMemberships: [],
            })),
          }
        : table,
    );
    const nameHeuristic = buildCatalogIndex({
      ...base,
      tables: withoutKeys,
      foreignKeys: [],
    });
    expect(
      complete(
        request("SELECT * FROM orders o JOIN customers c ON |", nameHeuristic),
      ).items,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ provenance: "name-heuristic" }),
        expect.objectContaining({ provenance: "type-heuristic" }),
      ]),
    );

    const uniqueCustomers = withoutKeys.map((table) =>
      table.id === "table:customers"
        ? {
            ...table,
            columns: table.columns.map((column) =>
              column.name.normalized === "id"
                ? { ...column, uniqueMemberships: ["unique:customers:id"] }
                : column,
            ),
          }
        : table,
    );
    const uniqueHeuristic = buildCatalogIndex({
      ...base,
      tables: uniqueCustomers,
      foreignKeys: [],
    });
    expect(
      complete(
        request(
          "SELECT * FROM orders o JOIN customers c ON |",
          uniqueHeuristic,
        ),
      ).items,
    ).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ provenance: "unique-key" }),
      ]),
    );

    const incompatibleCustomers = withoutKeys.map((table) =>
      table.id === "table:customers"
        ? {
            ...table,
            columns: table.columns.map((column) => ({
              ...column,
              name: {
                ...column.name,
                normalized: `customer_${column.name.normalized}`,
                raw: `customer_${column.name.raw}`,
              },
              type: { family: "text" as const, name: "text" },
            })),
          }
        : table,
    );
    const incompatible = buildCatalogIndex({
      ...base,
      tables: incompatibleCustomers,
      foreignKeys: [],
    });
    expect(
      complete(
        request("SELECT * FROM orders o JOIN customers c ON |", incompatible),
      ).items,
    ).toEqual([]);
  });

  it("covers context taxonomy branches", () => {
    const samples = [
      ["DELETE FROM us|", "delete-target"],
      ["UPDATE us|", "update-target"],
      ["SELECT *|", "select-list-continuation"],
      ["SELECT * FROM users GROUP BY |", "group-by-expression"],
      ["SELECT * FROM users HAVING |", "having-expression"],
      ["SELECT * FROM users ORDER BY |", "order-by-expression"],
      ["SELECT * FROM users JOIN |", "join-relation"],
    ] as const;
    for (const [sql, kind] of samples)
      expect(complete(request(sql)).context.kind).toBe(kind);
    expect(complete(request("INSERT INTO us|")).context.kind).toBe(
      "insert-target",
    );
    expect(
      complete(
        request("WITH recent AS (SELECT id FROM orders) SELECT * FROM x|"),
      ),
    ).toBeDefined();
    expect(
      complete(
        request(
          "SELECT * FROM customers c WHERE EXISTS (SELECT | FROM orders o)",
        ),
      ).items.length,
    ).toBeGreaterThan(0);
  });

  it("filters provider output by execution context and expected values", () => {
    const base = createCatalog().snapshot;
    const otherSchema = {
      id: "schema:other",
      catalogId: "other",
      databaseId: "db:other",
      name: identifier("private"),
    };
    const otherTable = {
      ...base.tables[0],
      id: "table:other:users",
      databaseId: "db:other",
      schemaId: otherSchema.id,
      name: {
        ...base.tables[0].name,
        raw: "private_users",
        normalized: "private_users",
      },
    };
    const catalog = buildCatalogIndex({
      ...base,
      databases: [
        { id: "db:test", name: identifier("test"), system: false },
        { id: "db:other", name: identifier("other"), system: false },
      ],
      schemas: [...base.schemas, otherSchema],
      tables: [...base.tables, otherTable],
      types: [
        {
          id: "type:state",
          name: identifier("state"),
          family: "enum",
          definition: `enum('active','in''active',"pending")`,
        },
      ],
    });
    const relation = context("SELECT * FROM |", catalog);
    const privateItems = provideRelationCandidates(
      { ...relation.request, executionContext: { database: "db:other" } },
      relation.context,
    );
    expect(privateItems.map((item) => item.displayLabel)).toContain(
      "private_users",
    );
    expect(privateItems.map((item) => item.displayLabel)).not.toContain(
      "users",
    );

    const sqlServer = request(
      "SELECT * FROM private.|",
      buildCatalogIndex({ ...catalog.snapshot, dialect: "sqlserver" }),
      "sqlserver",
    );
    const sqlServerContext = complete(sqlServer).context;
    expect(
      provideRelationCandidates(sqlServer, sqlServerContext).map(
        (item) => item.displayLabel,
      ),
    ).toEqual(["private_users"]);
    expect(
      provideRelationCandidates(sqlServer, {
        ...sqlServerContext,
        prefix: "missing",
      }),
    ).toEqual([]);

    const numeric = context("SELECT SUM(|) FROM users", catalog);
    const numericItems = provideColumnCandidates(numeric.request, {
      ...numeric.context,
      kind: "function-argument",
      expectedType: {
        type: { family: "integer", name: "integer" },
        reason: "function-argument",
        confidence: 1,
      },
    });
    expect(numericItems.map((item) => item.displayLabel)).toContain("id");
    expect(numericItems.map((item) => item.displayLabel)).not.toContain("name");

    const enumContext = context("SELECT * FROM users WHERE name = |", catalog);
    const enumValues = provideValueCandidates(enumContext.request, {
      ...enumContext.context,
      expectedType: {
        type: { family: "enum", name: "state" },
        reason: "comparison",
        confidence: 1,
      },
    });
    expect(enumValues.map((item) => item.displayLabel)).toEqual(
      expect.arrayContaining(["active", "in'active", "pending"]),
    );
    expect(
      provideTypeCandidates(enumContext.request, {
        ...enumContext.context,
        prefix: "missing",
        kind: "type",
      }),
    ).toEqual([]);
    const insert = context("INSERT INTO users (name)|", catalog);
    expect(
      provideInsertColumnCandidates(insert.request, insert.context).map(
        (item) => item.displayLabel,
      ),
    ).toEqual(["email"]);
  });

  it("uses fuzzy fallback and deterministic tie breakers", () => {
    const item = (label: string, sourceId: string): CompletionItem => ({
      from: 0,
      to: 0,
      insertText: label,
      displayLabel: label,
      kind: "column",
      score: 1,
      sourceId,
    });
    const ranked = rankCompletionItems(
      [item("customer_id", "b"), item("custom_name", "a"), item("zzzz", "c")],
      "cst",
    );
    expect(ranked.map((entry) => entry.displayLabel)).toEqual([
      "custom_name",
      "customer_id",
    ]);
    expect(
      rankCompletionItems([item("b", "b"), item("a", "a")], "")[0]
        ?.displayLabel,
    ).toBe("a");
    expect(
      rankCompletionItems([item("a", "b"), item("a", "a")], "")[0]?.sourceId,
    ).toBe("a");
    expect(
      rankCompletionItems(
        [item("same", "a"), { ...item("same", "b"), sourceId: undefined }],
        "",
      ),
    ).toHaveLength(2);
    expect(
      rankCompletionItems([item("alpha", "a")], "alpha")[0]?.features
        ?.prefixQuality,
    ).toBe(1);
    expect(getDialect("sqlite").parameterStyle).toBe("question");
  });

  it("measures ranking quality for empty, partial, and exact results", () => {
    const item = (displayLabel: string): CompletionItem => ({
      from: 0,
      to: 0,
      insertText: displayLabel,
      displayLabel,
      kind: "column",
      score: 1,
    });
    expect(evaluateRankingQuality([], [])).toEqual({
      top1: 0,
      mrr: 0,
      recallAt5: 1,
      recallAt10: 1,
      ndcgAt10: 1,
    });
    const quality = evaluateRankingQuality(
      [item("other"), item("expected")],
      ["expected", "missing"],
    );
    expect(quality.top1).toBe(0);
    expect(quality.mrr).toBe(0.5);
    expect(quality.recallAt5).toBe(0.5);
    expect(quality.ndcgAt10).toBeGreaterThan(0);
  });

  it("preserves quoted replacement ranges and reserved identifier rendering", () => {
    const catalog = createCatalog();
    const users = catalog.snapshot.tables[0];
    if (!users) throw new Error("Expected users table");
    const quotedCatalog = buildCatalogIndex({
      ...catalog.snapshot,
      tables: [
        {
          ...users,
          columns: [
            ...users.columns,
            {
              id: "column:users:select",
              tableId: users.id,
              ordinal: 3,
              name: {
                raw: "select",
                normalized: "select",
                quoted: false,
                quoteStyle: "none" as const,
              },
              type: { family: "text" as const, name: "text" },
              nullable: true,
              generated: false,
              identity: false,
              computed: false,
              primaryKey: false,
              foreignKeyMemberships: [],
              uniqueMemberships: [],
            },
          ],
        },
        ...catalog.snapshot.tables.slice(1),
      ],
    });
    const quoted = complete(
      request("SELECT u.se| FROM users u", quotedCatalog),
    );
    expect(
      quoted.items.find((item) => item.displayLabel === "u.select")?.insertText,
    ).toBe('"select"');
    expect(complete(request("SELECT u.| FROM users u")).context.range).toEqual({
      from: 9,
      to: 9,
    });
    expect(complete(request("SELECT u.!|")).context.qualifier).toBeUndefined();
    expect(complete(request("SELECT .|")).context.qualifier).toBe("select");
    expect(
      complete(request('SELECT u."na| FROM users u')).context.range.from,
    ).toBe(10);
    expect(
      complete(request("INSERT INTO missing (name) VALUES (|")).context
        .expectedType,
    ).toBeUndefined();
    const selectRequest = request("SELECT |");
    const select = complete(selectRequest);
    expect(
      provideSnippetCandidates(selectRequest, {
        ...select.context,
        kind: "select-projection",
      }),
    ).toEqual([]);
  });
});
