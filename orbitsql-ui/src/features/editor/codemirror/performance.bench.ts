import { sql } from "@codemirror/lang-sql";
import { syntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { test } from "vitest";
import { complete } from "../../sql-intelligence/completion/engine";
import { createSqlDocument } from "../../sql-intelligence/document/factory";
import { createCatalog } from "../../sql-intelligence/tests/fixtures";
import { mapCompletionResponse } from "./sql-completion-source";
import { codeMirrorDialect } from "./sql-language";

const catalog = createCatalog();
const document = createSqlDocument(
  "codemirror-benchmark",
  1,
  "SELECT u.na FROM users u WHERE u.",
  "postgresql",
);
const result = complete({
  document,
  catalog,
  position: document.text.length,
});
const state = EditorState.create({
  doc: document.text,
  extensions: [sql({ dialect: codeMirrorDialect("postgresql") })],
});

test("CodeMirror syntax tree lookup", ({ bench }) => {
  bench("resolve cursor syntax node", () => {
    syntaxTree(state).resolve(document.text.length, -1);
  });
});

test("semantic context and completion", ({ bench }) => {
  bench("complete against indexed catalog", () => {
    complete({
      document,
      catalog,
      position: document.text.length,
    });
  });
});

test("CodeMirror completion mapping", ({ bench }) => {
  bench("map completion result", () => {
    mapCompletionResponse(result);
  });
});
