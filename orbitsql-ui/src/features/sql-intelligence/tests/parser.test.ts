import { describe, expect, it } from "vitest";
import { getDialect } from "../dialect/registry";
import { tokenizeSql } from "../parser/tokens";

describe("error tolerant SQL tokenizer", () => {
  it("retains quoted identifiers and punctuation", () => {
    const tokens = tokenizeSql(
      'SELECT "user name" FROM [users]',
      getDialect("sqlserver"),
    );
    expect(
      tokens
        .filter((token) => token.kind === "quoted-identifier")
        .map((token) => token.normalized),
    ).toEqual(["user name", "users"]);
  });

  it("accepts incomplete input without throwing", () => {
    expect(() =>
      tokenizeSql(
        "SELECT * FROM users WHERE name = '",
        getDialect("postgresql"),
      ),
    ).not.toThrow();
    expect(
      tokenizeSql("SELECT * -- unfinished", getDialect("postgresql")),
    ).toEqual(expect.any(Array));
    expect(
      tokenizeSql("SELECT /* unfinished", getDialect("postgresql")),
    ).toEqual(expect.any(Array));
  });

  it("recognizes positional, named, and question parameters before identifiers", () => {
    const samples = [
      ["$1", "postgresql"],
      ["?", "mysql"],
      ["@user", "sqlserver"],
      [":user", "oracle"],
    ] as const;
    for (const [text, dialect] of samples) {
      expect(
        tokenizeSql(`SELECT ${text}`, getDialect(dialect))[1],
      ).toMatchObject({
        kind: "parameter",
        text,
      });
    }
  });
});
