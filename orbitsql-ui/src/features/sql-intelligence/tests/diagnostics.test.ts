import { describe, expect, it } from "vitest";
import { analyzeSql } from "../diagnostics/analyze";
import { createSqlDocument } from "../document/factory";
import { createCatalog } from "./fixtures";

describe("SQL diagnostics foundation", () => {
  it("reports unknown relations and accepts known relations", () => {
    const catalog = createCatalog();
    const issues = analyzeSql(
      createSqlDocument(
        "diagnostics",
        1,
        "SELECT * FROM missing",
        "postgresql",
      ),
      catalog,
    );
    expect(issues[0]?.code).toBe("unknown-relation");
    expect(
      analyzeSql(
        createSqlDocument(
          "diagnostics",
          2,
          "SELECT * FROM users",
          "postgresql",
        ),
        catalog,
      ),
    ).toEqual([]);
  });

  it("reports an empty document with a stable zero range", () => {
    expect(
      analyzeSql(
        createSqlDocument("diagnostics", 3, "  ", "postgresql"),
        createCatalog(),
      ),
    ).toEqual([
      {
        code: "empty-statement",
        message: "The SQL document is empty",
        severity: "info",
        range: { from: 0, to: 0 },
      },
    ]);
    expect(
      analyzeSql(
        createSqlDocument("diagnostics", 4, "SELECT * FROM", "postgresql"),
        createCatalog(),
      ),
    ).toEqual([]);
    expect(
      analyzeSql(
        createSqlDocument("diagnostics", 5, "SELECT * FROM (", "postgresql"),
        createCatalog(),
      ),
    ).toEqual([]);
  });

  it("resolves schema-qualified relations, CTEs, and aliases", () => {
    const catalog = createCatalog();
    expect(
      analyzeSql(
        createSqlDocument(
          "diagnostics",
          6,
          "WITH recent AS (SELECT id FROM users) SELECT u.id FROM public.users u",
          "postgresql",
        ),
        catalog,
      ),
    ).toEqual([]);
    expect(
      analyzeSql(
        createSqlDocument(
          "diagnostics",
          7,
          "SELECT missing.id FROM users",
          "postgresql",
        ),
        catalog,
      ).map((issue) => issue.code),
    ).toContain("unresolved-qualifier");
    expect(
      analyzeSql(
        createSqlDocument(
          "diagnostics",
          10,
          "WITH recent AS (SELECT id FROM users) SELECT * FROM recent",
          "postgresql",
        ),
        catalog,
      ),
    ).toEqual([]);
  });

  it("recovers from incomplete and parenthesized relation sources", () => {
    const catalog = createCatalog();
    expect(
      analyzeSql(
        createSqlDocument(
          "diagnostics",
          8,
          "WITH broken ( SELECT * FROM users JOIN ( SELECT * FROM users )",
          "postgresql",
        ),
        catalog,
      ),
    ).toEqual([]);
    expect(
      analyzeSql(
        createSqlDocument(
          "diagnostics",
          9,
          "SELECT * FROM users JOIN",
          "postgresql",
        ),
        catalog,
      ),
    ).toEqual([]);
  });
});
