import type { CatalogIndex } from "../catalog";
import { getDialect } from "../dialect/registry";
import { tokenizeSql } from "../parser/tokens";
import { buildCompletionContext } from "../semantic/context";
import {
  buildRelationshipGraph,
  type RelationshipGraph,
} from "../semantic/relations";
import {
  provideAliasCandidates,
  provideColumnCandidates,
  provideFunctionCandidates,
  provideInsertColumnCandidates,
  provideJoinCandidates,
  provideKeywordCandidates,
  provideOperatorCandidates,
  provideRelationCandidates,
  provideSchemaCandidates,
  provideSelectListContinuationCandidates,
  provideTypeCandidates,
  provideUpdateColumnCandidates,
  provideValueCandidates,
} from "./providers";
import { rankCompletionItems } from "./ranking";
import { provideSnippetCandidates } from "./snippets";
import type {
  CompletionContext,
  CompletionRequest,
  CompletionResponse,
} from "./types";

const relationshipGraphs = new WeakMap<CatalogIndex, RelationshipGraph>();

export class CompletionCancelledError extends Error {
  override name = "AbortError";

  constructor() {
    super("Completion request was aborted");
  }
}

function checkCancelled(signal: AbortSignal | undefined): void {
  if (signal?.aborted) throw new CompletionCancelledError();
}

function relationships(request: CompletionRequest): RelationshipGraph {
  const cached = relationshipGraphs.get(request.catalog);
  if (cached) return cached;
  const graph = buildRelationshipGraph(request.catalog);
  relationshipGraphs.set(request.catalog, graph);
  return graph;
}

function candidates(
  request: CompletionRequest,
  context: CompletionContext,
  relationGraph: RelationshipGraph | undefined,
) {
  switch (context.kind) {
    case "statement-start":
    case "keyword":
      return [
        ...provideKeywordCandidates(request, context),
        ...provideSnippetCandidates(request, context),
      ];
    case "select-list-continuation":
      return provideSelectListContinuationCandidates(request, context);
    case "from-relation":
    case "join-relation":
    case "insert-target":
    case "update-target":
    case "delete-target":
      return provideRelationCandidates(request, context);
    case "relation-alias":
    case "select-alias":
      return [];
    case "schema-name":
      return provideSchemaCandidates(request, context);
    case "join-condition":
      return context.qualifier
        ? provideColumnCandidates(request, context)
        : [
            ...provideJoinCandidates(
              request,
              context,
              relationGraph ?? relationships(request),
            ),
            ...(context.operatorExpected
              ? provideOperatorCandidates(request, context)
              : []),
          ];
    case "type":
      return provideTypeCandidates(request, context);
    case "insert-columns":
      return provideInsertColumnCandidates(request, context);
    case "update-set-column":
      return provideUpdateColumnCandidates(request, context);
    case "insert-values":
    case "update-set-value":
    case "function-argument":
      return [
        ...provideColumnCandidates(request, context),
        ...provideValueCandidates(request, context),
        ...provideFunctionCandidates(request, context),
      ];
    case "order-by-expression":
    case "group-by-expression":
      return [
        ...provideColumnCandidates(request, context),
        ...provideAliasCandidates(request, context),
        ...provideFunctionCandidates(request, context),
      ];
    case "select-projection":
    case "column":
    case "qualified-column":
    case "where-expression":
    case "having-expression":
      return [
        ...provideColumnCandidates(request, context),
        ...(context.qualifier ? [] : provideValueCandidates(request, context)),
        ...(context.operatorExpected
          ? provideOperatorCandidates(request, context)
          : []),
        ...(!context.qualifier &&
        (context.kind === "select-projection" ||
          context.kind === "where-expression" ||
          context.kind === "having-expression")
          ? provideFunctionCandidates(request, context)
          : []),
      ];
    case "cte-reference":
    case "none":
      return [];
  }
}

export function complete(request: CompletionRequest): CompletionResponse {
  checkCancelled(request.signal);
  const context = buildCompletionContext(
    request.document,
    request.catalog,
    request.position,
    request.syntax,
    request.tokens,
    request.executionContext,
  );
  checkCancelled(request.signal);
  const relationGraph =
    context.kind === "join-condition" && !context.qualifier
      ? relationships(request)
      : undefined;
  const ranked = rankCompletionItems(
    candidates(request, context, relationGraph),
    context.prefix,
  );
  checkCancelled(request.signal);
  const items =
    request.maxItems === undefined
      ? ranked
      : ranked.slice(0, Math.max(0, request.maxItems));
  return {
    documentId: request.document.id,
    documentVersion: request.document.version,
    catalogVersion: request.catalog.snapshot.version,
    context,
    items,
    isIncomplete:
      request.maxItems !== undefined && ranked.length > items.length,
  };
}

export function isCompletionCurrent(
  response: CompletionResponse,
  documentId: string,
  documentVersion: number,
  catalogVersion: string,
): boolean {
  return (
    response.documentId === documentId &&
    response.documentVersion === documentVersion &&
    response.catalogVersion === catalogVersion
  );
}

export function tokenizeForCompletion(request: CompletionRequest) {
  return tokenizeSql(
    request.document.text,
    getDialect(request.document.dialect),
  );
}
