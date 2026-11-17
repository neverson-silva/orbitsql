import type { CatalogSnapshot } from "../../sql-intelligence/catalog";
import type {
  CompletionExecutionContext,
  CompletionResponse,
  SyntaxSnapshot,
} from "../../sql-intelligence/completion/types";

export type RuntimeChange = Readonly<{
  from: number;
  to: number;
  insert: string;
}>;

export type RuntimeCompletionRequest = Readonly<{
  type: "complete";
  requestId: string;
  documentId: string;
  documentVersion: number;
  catalogKey: string;
  position: number;
  executionContext?: CompletionExecutionContext;
  syntax?: SyntaxSnapshot;
  maxItems: number;
  cancelBuffer?: SharedArrayBuffer;
}>;

export type RuntimeMessage =
  | Readonly<{
      type: "register-catalog";
      catalogKey: string;
      snapshot: CatalogSnapshot;
    }>
  | Readonly<{
      type: "release-catalog";
      catalogKey: string;
    }>
  | Readonly<{
      type: "open-document";
      documentId: string;
      documentVersion: number;
      catalogKey: string;
      dialect: CatalogSnapshot["dialect"];
      text: string;
    }>
  | Readonly<{
      type: "apply-document-changes";
      documentId: string;
      fromVersion: number;
      toVersion: number;
      changes: readonly RuntimeChange[];
    }>
  | Readonly<{
      type: "close-document";
      documentId: string;
    }>
  | RuntimeCompletionRequest
  | Readonly<{
      type: "cancel";
      requestId: string;
    }>;

export type RuntimeWorkerMessage =
  | Readonly<{
      type: "catalog-ready";
      catalogKey: string;
    }>
  | Readonly<{
      type: "completion-result";
      requestId: string;
      response: CompletionResponse;
    }>
  | Readonly<{
      type: "runtime-error";
      requestId?: string;
      message: string;
    }>;

export function applyRuntimeChanges(
  text: string,
  changes: readonly RuntimeChange[],
): string {
  let offset = 0;
  let result = text;
  for (const change of changes) {
    const from = change.from + offset;
    const to = change.to + offset;
    result = `${result.slice(0, from)}${change.insert}${result.slice(to)}`;
    offset += change.insert.length - (change.to - change.from);
  }
  return result;
}
