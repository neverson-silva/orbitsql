import { test } from "vitest";
import { buildCatalogIndex, type CatalogSnapshot } from "../catalog";
import { complete } from "../completion/engine";
import { createSqlDocument } from "../document/factory";

function identifier(name: string) {
  return {
    raw: name,
    normalized: name,
    quoted: false as const,
    quoteStyle: "none" as const,
  };
}

function catalogWithTables(tableCount: number) {
  const tables = Array.from({ length: tableCount }, (_, index) => {
    const name = `table_${index}`;
    return {
      id: `table:${index}`,
      databaseId: "db:benchmark",
      schemaId: "schema:public",
      name: identifier(name),
      columns: [
        {
          id: `column:${index}:id`,
          tableId: `table:${index}`,
          ordinal: 0,
          name: identifier("id"),
          type: { family: "integer" as const, name: "integer" },
          nullable: false,
          generated: false,
          identity: true,
          computed: false,
          primaryKey: true,
          foreignKeyMemberships: [],
          uniqueMemberships: [],
        },
      ],
      foreignKeys: [],
      uniqueConstraints: [],
      checkConstraints: [],
      indexes: [],
      temporary: false,
    };
  });
  const snapshot: CatalogSnapshot = {
    schemaVersion: 1,
    id: `benchmark:${tableCount}`,
    connectionId: "connection:benchmark",
    databaseId: "db:benchmark",
    dialect: "postgresql",
    version: String(tableCount),
    generatedAt: "2026-01-01T00:00:00.000Z",
    databases: [],
    schemas: [
      {
        id: "schema:public",
        databaseId: "db:benchmark",
        name: identifier("public"),
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
    foreignKeys: [],
    indexes: [],
    constraints: [],
  };
  return buildCatalogIndex(snapshot);
}

for (const size of [20, 500, 5_000, 20_000]) {
  test(`qualified relation completion with ${size} objects`, ({ bench }) => {
    const catalog = catalogWithTables(size);
    bench("completion", () => {
      const document = createSqlDocument(
        "bench",
        1,
        `SELECT * FROM table_${size - 1}`,
        "postgresql",
      );
      complete({ document, catalog, position: document.text.length });
    });
  });
}

for (const lines of [10, 100, 1_000, 10_000]) {
  test(`multi-statement document with ${lines} lines`, ({ bench }) => {
    const text = Array.from(
      { length: lines },
      (_, index) => `SELECT ${index};`,
    ).join("\n");
    const catalog = catalogWithTables(20);
    bench("completion", () => {
      const document = createSqlDocument(
        "document-bench",
        lines,
        text,
        "postgresql",
      );
      complete({ document, catalog, position: text.length });
    });
  });
}
