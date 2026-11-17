import { CompletionContext } from "@codemirror/autocomplete";
import { MSSQL, PostgreSQL, sql } from "@codemirror/lang-sql";
import { EditorState } from "@codemirror/state";
import { describe, expect, it, vi } from "vitest";
import { buildCatalogIndex } from "../../sql-intelligence/catalog";
import { cursorFromMarker } from "../../sql-intelligence/document/test-utils";
import { createCatalog } from "../../sql-intelligence/tests/fixtures";
import { createSqlCompletionSource } from "./sql-completion-source";

describe("CodeMirror SQL completion adapter", () => {
  it("maps the pure engine range and options", () => {
    const marker = cursorFromMarker("SELECT * FROM us|");
    const state = EditorState.create({ doc: marker.text, extensions: [sql()] });
    const source = createSqlCompletionSource(createCatalog());
    const result = source(new CompletionContext(state, marker.position, false));
    expect(result).not.toBeNull();
    expect(result && "from" in result ? result.from : -1).toBe(14);
    expect(
      result && "options" in result ? result.options[0]?.label : undefined,
    ).toBe("users");
  });

  it("offers relations after an empty FROM clause", () => {
    const text = "SELECT * FROM ";
    const state = EditorState.create({ doc: text, extensions: [sql()] });
    const source = createSqlCompletionSource(createCatalog());
    const result = source(new CompletionContext(state, text.length, false));
    expect(result && "options" in result ? result.options : []).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ label: "users" }),
        expect.objectContaining({ label: "orders" }),
        expect.objectContaining({ label: "public" }),
      ]),
    );
  });

  it("offers clause continuations after a completed wildcard", () => {
    const text = "SELECT *";
    const state = EditorState.create({ doc: text, extensions: [sql()] });
    const source = createSqlCompletionSource(createCatalog());
    const result = source(new CompletionContext(state, text.length, false));
    expect(result && "options" in result ? result.options : []).toEqual(
      expect.arrayContaining([expect.objectContaining({ label: "FROM" })]),
    );
    expect(
      result && "options" in result
        ? result.options.some((option) => option.label === "ABS")
        : false,
    ).toBe(false);
  });

  it("offers SQL Server schemas before tables after FROM", () => {
    const catalog = buildCatalogIndex({
      ...createCatalog().snapshot,
      dialect: "sqlserver",
    });
    const text = "SELECT * FROM ";
    const state = EditorState.create({
      doc: text,
      extensions: [sql({ dialect: MSSQL })],
    });
    const source = createSqlCompletionSource(catalog);
    const result = source(new CompletionContext(state, text.length, false));
    expect(result && "options" in result ? result.options : []).toEqual([
      expect.objectContaining({ label: "public", type: "schema" }),
    ]);
  });

  it("offers SQL Server schemas immediately after the FROM keyword", () => {
    const catalog = buildCatalogIndex({
      ...createCatalog().snapshot,
      dialect: "sqlserver",
    });
    const text = "SELECT * FROM";
    const state = EditorState.create({
      doc: text,
      extensions: [sql({ dialect: MSSQL })],
    });
    const source = createSqlCompletionSource(catalog);
    const result = source(new CompletionContext(state, text.length, false));
    expect(result && "from" in result ? result.from : -1).toBe(text.length);
    expect(result && "options" in result ? result.options : []).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ label: "public", type: "schema" }),
      ]),
    );
  });

  it("uses the maintained Lezer tree at the cursor", () => {
    const marker = cursorFromMarker("SELECT u.| FROM users u");
    const state = EditorState.create({
      doc: marker.text,
      extensions: [sql({ dialect: PostgreSQL })],
    });
    const source = createSqlCompletionSource(createCatalog());
    const result = source(new CompletionContext(state, marker.position, true));
    expect(result && "options" in result ? result.options : []).toEqual(
      expect.arrayContaining([expect.objectContaining({ label: "u.id" })]),
    );
    expect(
      result && "options" in result
        ? result.options.find((option) => option.label === "u.id")?.apply
        : undefined,
    ).toBe("id");
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
      dialect: "sqlserver",
      schemas: [schema],
      tables: catalog.snapshot.tables.map((table) => ({
        ...table,
        schemaId: schema.id,
      })),
    });
    const text = "SELECT * FROM [trinus-uau].";
    const state = EditorState.create({
      doc: text,
      extensions: [sql({ dialect: MSSQL })],
    });
    const source = createSqlCompletionSource(scopedCatalog);
    const result = source(new CompletionContext(state, text.length, false));
    expect(result && "options" in result ? result.options : []).toEqual(
      expect.arrayContaining([expect.objectContaining({ label: "users" })]),
    );
  });

  it("replaces a partial bracket-quoted schema without duplicating the bracket", () => {
    const catalog = buildCatalogIndex({
      ...createCatalog().snapshot,
      dialect: "sqlserver",
      schemas: [
        {
          ...createCatalog().snapshot.schemas[0],
          id: "schema:trinus-uau",
          name: {
            ...createCatalog().snapshot.schemas[0].name,
            raw: "trinus-uau",
            normalized: "trinus-uau",
          },
        },
      ],
    });
    const text = "SELECT * FROM [tri";
    const state = EditorState.create({
      doc: text,
      extensions: [sql({ dialect: MSSQL })],
    });
    const source = createSqlCompletionSource(catalog);
    const result = source(new CompletionContext(state, text.length, false));
    const option =
      result && "options" in result
        ? result.options.find((entry) => entry.label === "trinus-uau")
        : undefined;
    expect(result && "from" in result ? result.from : -1).toBe(14);
    expect(option?.apply).toBe("[trinus-uau]");
  });

  it("replaces a partial bracket-quoted table without duplicating the bracket", () => {
    const base = createCatalog().snapshot;
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
      schemas: [schema],
      tables: base.tables.map((table) => ({
        ...table,
        schemaId: schema.id,
      })),
    });
    const text = "SELECT * FROM [trinus-uau].[cus";
    const state = EditorState.create({
      doc: text,
      extensions: [sql({ dialect: MSSQL })],
    });
    const source = createSqlCompletionSource(catalog);
    const result = source(new CompletionContext(state, text.length, false));
    const option =
      result && "options" in result
        ? result.options.find((entry) => entry.label === "customers")
        : undefined;
    expect(result && "from" in result ? result.from : -1).toBe(27);
    expect(option?.apply).toBe("customers");
  });

  it("does not reopen schema suggestions after a relation alias", () => {
    const catalog = buildCatalogIndex({
      ...createCatalog().snapshot,
      dialect: "sqlserver",
    });
    const text = "SELECT * FROM [trinus-uau].customer c";
    const state = EditorState.create({
      doc: text,
      extensions: [sql({ dialect: MSSQL })],
    });
    const source = createSqlCompletionSource(catalog);
    const result = source(new CompletionContext(state, text.length, false));
    expect(result && "options" in result ? result.options : []).toEqual([]);
  });

  it("does not offer SQL completion inside a maintained string node", () => {
    const marker = cursorFromMarker("SELECT 'abc|'");
    const state = EditorState.create({
      doc: marker.text,
      extensions: [sql({ dialect: PostgreSQL })],
    });
    const source = createSqlCompletionSource(createCatalog());
    const result = source(new CompletionContext(state, marker.position, true));
    expect(result).toBeNull();
  });

  it("maps SQL snippets to CodeMirror snippet applications", () => {
    const marker = cursorFromMarker("s|");
    const state = EditorState.create({ doc: marker.text, extensions: [sql()] });
    const source = createSqlCompletionSource(createCatalog());
    const result = source(new CompletionContext(state, marker.position, false));
    const option =
      result && "options" in result
        ? result.options.find((entry) => entry.label === "SELECT query")
        : undefined;
    expect(option?.apply).toBeTypeOf("function");
    const dispatch = vi.fn();
    if (typeof option?.apply !== "function")
      throw new Error("Expected snippet application");
    const apply = option.apply;
    const editor = { state, dispatch } as unknown as Parameters<
      typeof apply
    >[0];
    apply(editor, option, marker.position - 1, marker.position);
    expect(dispatch).toHaveBeenCalledOnce();
    expect(dispatch.mock.calls[0]?.[0].newDoc.toString()).toContain("SELECT ");
  });
});
