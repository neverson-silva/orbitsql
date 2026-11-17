import { describe, expect, it } from "vitest";
import { buildCatalogIndex, type CatalogSnapshot } from "../catalog";
import type { CatalogTable } from "../catalog/types";
import { complete } from "../completion/engine";
import { createSqlDocument } from "../document/factory";

function benchmarkSnapshot(tableCount: number): CatalogSnapshot {
  const tables: CatalogTable[] = Array.from(
    { length: tableCount },
    (_, index) => ({
      id: `table:${index}`,
      databaseId: "database:benchmark",
      schemaId: "schema:public",
      name: {
        raw: `table_${index}`,
        normalized: `table_${index}`,
        quoted: false,
        quoteStyle: "none",
      },
      columns: [
        {
          id: `column:${index}:1`,
          tableId: `table:${index}`,
          ordinal: 1,
          name: {
            raw: "id",
            normalized: "id",
            quoted: false,
            quoteStyle: "none",
          },
          type: { family: "integer", name: "integer" },
          nullable: false,
          generated: false,
          identity: false,
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
    }),
  );
  return {
    schemaVersion: 1,
    id: "catalog:benchmark",
    connectionId: "connection:benchmark",
    databaseId: "database:benchmark",
    dialect: "postgresql",
    version: "1",
    generatedAt: "1970-01-01T00:00:00.000Z",
    databases: [],
    schemas: [
      {
        id: "schema:public",
        databaseId: "database:benchmark",
        name: {
          raw: "public",
          normalized: "public",
          quoted: false,
          quoteStyle: "none",
        },
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
}

function percentile(values: readonly number[], fraction: number): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[
    Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)
  ];
}

describe("completion performance targets", () => {
  it("keeps qualified completion within the hot-path budget", () => {
    const catalog = buildCatalogIndex(benchmarkSnapshot(5000));
    const document = createSqlDocument(
      "performance",
      1,
      "SELECT t.| FROM table_4999 t",
      "postgresql",
    );
    const samples: number[] = [];
    for (let index = 0; index < 120; index += 1) {
      const start = performance.now();
      complete({ document, catalog, position: 9 });
      samples.push(performance.now() - start);
    }
    expect(percentile(samples, 0.5)).toBeLessThan(5);
    expect(percentile(samples, 0.95)).toBeLessThan(20);
    expect(percentile(samples, 0.99)).toBeLessThan(50);
  });
});
