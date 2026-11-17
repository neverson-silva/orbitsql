import { describe, expect, it } from "vitest";
import { complete } from "../completion/engine";
import { evaluateRankingQuality } from "../completion/quality";
import { createSqlDocument } from "../document/factory";
import { cursorFromMarker } from "../document/test-utils";
import { createCatalog } from "./fixtures";

describe("ranking quality", () => {
  it("handles empty and unretrieved relevance sets", () => {
    expect(evaluateRankingQuality([], [])).toEqual({
      top1: 0,
      mrr: 0,
      recallAt5: 1,
      recallAt10: 1,
      ndcgAt10: 1,
    });
    expect(evaluateRankingQuality([], ["missing"])).toEqual({
      top1: 0,
      mrr: 0,
      recallAt5: 0,
      recallAt10: 0,
      ndcgAt10: 0,
    });
    const item = {
      from: 0,
      to: 0,
      insertText: "id",
      displayLabel: "id",
      kind: "column" as const,
      score: 1,
    };
    expect(evaluateRankingQuality([item], ["id"])).toEqual({
      top1: 1,
      mrr: 1,
      recallAt5: 1,
      recallAt10: 1,
      ndcgAt10: 1,
    });
  });

  it("measures annotated semantic completion fixtures", () => {
    const fixtures = [
      {
        sql: "SELECT u.na| FROM users u",
        expected: ["u.name"],
      },
      {
        sql: "SELECT * FROM us|",
        expected: ["users"],
      },
      {
        sql: "SELECT o.tot| FROM orders o",
        expected: ["o.total"],
      },
    ];
    const metrics = fixtures.map(({ sql, expected }) => {
      const cursor = cursorFromMarker(sql);
      const document = createSqlDocument(
        "ranking",
        1,
        cursor.text,
        "postgresql",
      );
      const result = complete({
        document,
        catalog: createCatalog(),
        position: cursor.position,
      });
      return evaluateRankingQuality(result.items, expected);
    });
    const average = (key: keyof (typeof metrics)[number]) =>
      metrics.reduce((sum, metric) => sum + metric[key], 0) / metrics.length;
    expect(average("top1")).toBeGreaterThanOrEqual(0.66);
    expect(average("mrr")).toBeGreaterThanOrEqual(0.66);
    expect(average("recallAt5")).toBeGreaterThanOrEqual(0.66);
    expect(average("recallAt10")).toBeGreaterThanOrEqual(0.66);
    expect(average("ndcgAt10")).toBeGreaterThanOrEqual(0.66);
  });
});
