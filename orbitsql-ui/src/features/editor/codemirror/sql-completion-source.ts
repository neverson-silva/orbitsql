import type {
  Completion,
  CompletionResult,
  CompletionSource,
} from "@codemirror/autocomplete";
import { snippet } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import type { CatalogIndex } from "../../sql-intelligence/catalog";
import {
  complete,
  isCompletionCurrent,
} from "../../sql-intelligence/completion/engine";
import type {
  CompletionExecutionContext,
  CompletionKind,
  CompletionResponse,
} from "../../sql-intelligence/completion/types";
import {
  getDialect,
  normalizeIdentifier,
} from "../../sql-intelligence/dialect/registry";
import { createSqlDocument } from "../../sql-intelligence/document/factory";
import type {
  SqlToken,
  SqlTokenKind,
} from "../../sql-intelligence/parser/tokens";
import type { SqlIntelligenceRuntime } from "./sql-intelligence-runtime";

function syntaxTokens(
  state: Parameters<CompletionSource>[0],
  tree: ReturnType<typeof syntaxTree>,
  dialectId: CatalogIndex["snapshot"]["dialect"],
): readonly SqlToken[] {
  const dialect = getDialect(dialectId);
  const leaves: SqlToken[] = [];
  tree.iterate({
    enter(node) {
      if (node.node.firstChild) return;
      const text = state.state.doc.sliceString(node.from, node.to);
      if (node.name === "LineComment" || node.name === "BlockComment") return;
      const quoted = node.name === "QuotedIdentifier";
      const kind: SqlTokenKind = quoted
        ? "quoted-identifier"
        : node.name === "Keyword"
          ? "keyword"
          : node.name === "Number"
            ? "number"
            : node.name === "String"
              ? "string"
              : node.name === "Operator" || node.name === "⚠"
                ? "operator"
                : ".,();".includes(node.name)
                  ? "punctuation"
                  : "identifier";
      leaves.push({
        kind,
        text,
        normalized:
          kind === "quoted-identifier" ||
          kind === "identifier" ||
          kind === "keyword"
            ? normalizeIdentifier(text, dialect)
            : text,
        from: node.from,
        to: node.to,
        quoteStyle: quoted
          ? text.startsWith("[")
            ? "bracket"
            : text.startsWith("`")
              ? "backtick"
              : "double"
          : undefined,
      });
    },
  });
  const result: SqlToken[] = [];
  for (let index = 0; index < leaves.length; index += 1) {
    const current = leaves[index];
    if (
      dialect.parameterStyle === "$n" &&
      current.text === "$" &&
      leaves[index + 1]?.kind === "number" &&
      leaves[index + 1]?.from === current.to
    ) {
      const number = leaves[index + 1];
      result.push({
        kind: "parameter",
        text: state.state.doc.sliceString(current.from, number.to),
        normalized: state.state.doc.sliceString(current.from, number.to),
        from: current.from,
        to: number.to,
      });
      index += 1;
      continue;
    }
    result.push(current);
  }
  return result;
}

function completionType(kind: CompletionKind): string {
  if (kind === "column" || kind === "alias") return "property";
  if (kind === "table" || kind === "view" || kind === "cte") return "type";
  if (kind === "join") return "operator";
  return kind;
}

export function mapCompletionResponse(
  result: CompletionResponse,
): CompletionResult {
  const options: Completion[] = result.items.map((item) => ({
    label: item.displayLabel,
    displayLabel: item.displayLabel,
    apply: item.snippet ? snippet(item.snippet) : item.insertText,
    detail: item.detail,
    type: completionType(item.kind),
    boost: Math.max(-99, Math.min(99, item.score - 100)),
  }));
  return {
    from: result.context.range.from,
    to: result.context.range.to,
    options,
    validFor: /^[\p{L}\p{N}_$]*$/u,
  };
}

function isSuppressedSyntaxNode(nodeName: string): boolean {
  return (
    nodeName === "String" ||
    nodeName === "LineComment" ||
    nodeName === "BlockComment"
  );
}

function syntaxSnapshot(
  context: Parameters<CompletionSource>[0],
  tree: ReturnType<typeof syntaxTree>,
) {
  const syntaxNode = tree.resolve(context.pos, -1);
  return {
    syntaxNode,
    snapshot: {
      source: "codemirror" as const,
      rootName: tree.type.name,
      nodeName: syntaxNode.name,
      nodeFrom: syntaxNode.from,
      nodeTo: syntaxNode.to,
      errorTolerant: true,
    },
  };
}

function createSynchronousSqlCompletionSource(
  catalog: CatalogIndex,
  documentId: string,
  documentVersion: () => number,
  executionContext?: CompletionExecutionContext,
): CompletionSource {
  return (context): CompletionResult | null => {
    const tree = syntaxTree(context.state);
    const { syntaxNode, snapshot } = syntaxSnapshot(context, tree);
    if (isSuppressedSyntaxNode(syntaxNode.name)) return null;
    const text = context.state.doc.toString();
    const tokens = syntaxTokens(context, tree, catalog.snapshot.dialect);
    const controller = new AbortController();
    context.addEventListener("abort", () => controller.abort(), {
      onDocChange: true,
    });
    const document = createSqlDocument(
      documentId,
      documentVersion(),
      text,
      catalog.snapshot.dialect,
    );
    const result = complete({
      document,
      catalog,
      position: context.pos,
      signal: controller.signal,
      executionContext,
      tokens,
      syntax: snapshot,
    });
    if (result.context.kind === "none") return null;
    if (
      !context.explicit &&
      result.context.prefix.length === 0 &&
      syntaxNode.name === "Script" &&
      result.context.kind === "statement-start"
    )
      return null;
    return mapCompletionResponse(result);
  };
}

function createRuntimeSqlCompletionSource(
  catalog: CatalogIndex,
  runtime: SqlIntelligenceRuntime,
  documentId: string,
  documentVersion: () => number,
  fallback: CompletionSource,
  executionContext?: CompletionExecutionContext,
): CompletionSource {
  return async (context): Promise<CompletionResult | null> => {
    const tree = syntaxTree(context.state);
    const { syntaxNode, snapshot } = syntaxSnapshot(context, tree);
    if (isSuppressedSyntaxNode(syntaxNode.name)) return null;
    const requestVersion = documentVersion();
    const handle = runtime.complete({
      documentId,
      documentVersion: requestVersion,
      position: context.pos,
      executionContext,
      syntax: snapshot,
    });
    context.addEventListener("abort", handle.cancel, { onDocChange: true });
    try {
      const result = await handle.promise;
      if (
        !isCompletionCurrent(
          result,
          documentId,
          requestVersion,
          catalog.snapshot.version,
        )
      ) {
        return null;
      }
      if (result.context.kind === "none") return null;
      if (
        !context.explicit &&
        result.context.prefix.length === 0 &&
        syntaxNode.name === "Script" &&
        result.context.kind === "statement-start"
      ) {
        return null;
      }
      return mapCompletionResponse(result);
    } catch (error) {
      if (error instanceof Error && error.name === "AbortError") return null;
      return fallback(context);
    }
  };
}

export function createSqlCompletionSource(
  catalog: CatalogIndex,
  documentId = "editor-document",
  documentVersion: () => number = () => 0,
  executionContext?: CompletionExecutionContext,
  runtime?: SqlIntelligenceRuntime,
): CompletionSource {
  const fallback = createSynchronousSqlCompletionSource(
    catalog,
    documentId,
    documentVersion,
    executionContext,
  );
  return runtime
    ? createRuntimeSqlCompletionSource(
        catalog,
        runtime,
        documentId,
        documentVersion,
        fallback,
        executionContext,
      )
    : fallback;
}
