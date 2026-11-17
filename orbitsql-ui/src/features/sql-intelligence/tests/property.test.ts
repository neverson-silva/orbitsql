import fc from "fast-check";
import { describe, expect, it } from "vitest";
import { complete } from "../completion/engine";
import {
  getDialect,
  normalizeIdentifier,
  quoteIdentifier,
} from "../dialect/registry";
import { createSqlDocument } from "../document/factory";
import { isValidRange } from "../document/validation";
import { createCatalog } from "./fixtures";

describe("SQL intelligence invariants", () => {
  it("never throws for arbitrary incomplete SQL and always returns a valid edit range", () => {
    fc.assert(
      fc.property(fc.string({ maxLength: 160 }), (text) => {
        const document = createSqlDocument(
          "property",
          text.length,
          text,
          "postgresql",
        );
        const response = complete({
          document,
          catalog: createCatalog(),
          position: text.length,
        });
        expect(
          response.items.every((item) => isValidRange(item, text.length)),
        ).toBe(true);
      }),
      { numRuns: 120 },
    );
  });

  it("keeps identifier quoting deterministic across all supported quote styles", () => {
    for (const dialect of [
      "postgresql",
      "mysql",
      "sqlserver",
      "oracle",
      "sqlite",
    ] as const) {
      const definition = getDialect(dialect);
      const quoted = quoteIdentifier(
        `a${definition.quoting.close}b`,
        definition,
      );
      expect(normalizeIdentifier(quoted, definition)).toBe(
        `a${definition.quoting.close}b`,
      );
    }
  });
});
