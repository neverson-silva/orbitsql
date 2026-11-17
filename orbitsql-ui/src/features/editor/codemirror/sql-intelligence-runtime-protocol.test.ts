import { describe, expect, it } from "vitest";
import { applyRuntimeChanges } from "./sql-intelligence-runtime-protocol";

describe("SQL intelligence runtime protocol", () => {
  it("applies CodeMirror changes in their original document coordinate space", () => {
    expect(
      applyRuntimeChanges("SELECT users", [
        { from: 0, to: 6, insert: "UPDATE" },
        { from: 7, to: 12, insert: "customers" },
      ]),
    ).toBe("UPDATE customers");
  });

  it("handles insertions and deletions in one transaction", () => {
    expect(
      applyRuntimeChanges("abcdef", [
        { from: 1, to: 1, insert: "XY" },
        { from: 4, to: 6, insert: "" },
      ]),
    ).toBe("aXYbcd");
  });
});
