import type { CatalogIndex } from "../catalog";
import type { SqlDialect } from "../dialect/types";
import type { SqlDocument, TextRange } from "../document/types";
import type { SqlToken } from "../parser/tokens";
import type { Scope, ScopeGraph } from "../semantic/types";

export type CompletionKind =
  | "column"
  | "table"
  | "view"
  | "alias"
  | "cte"
  | "function"
  | "keyword"
  | "type"
  | "operator"
  | "join"
  | "snippet"
  | "value"
  | "schema"
  | "procedure";

export type CompletionProvenance =
  | "catalog"
  | "real-foreign-key"
  | "unique-key"
  | "name-heuristic"
  | "type-heuristic"
  | "dialect"
  | "syntax";

export type RankingFeatures = Readonly<{
  prefixQuality: number;
  semanticFit: number;
  scopeDistance: number;
  qualifierMatch: number;
  typeCompatibility: number;
  relationshipConfidence: number;
  objectKindPrior: number;
  schemaAffinity: number;
  usageSignal: number;
  ambiguityPenalty: number;
}>;

export type CompletionItem = Readonly<{
  from: number;
  to: number;
  insertText: string;
  displayLabel: string;
  kind: CompletionKind;
  detail?: string;
  score: number;
  typeName?: string;
  sourceId?: string;
  provenance?: CompletionProvenance;
  features?: RankingFeatures;
  snippet?: string;
}>;

export type CompletionContextKind =
  | "statement-start"
  | "select-projection"
  | "select-list-continuation"
  | "from-relation"
  | "relation-alias"
  | "select-alias"
  | "join-relation"
  | "column"
  | "qualified-column"
  | "where-expression"
  | "having-expression"
  | "group-by-expression"
  | "order-by-expression"
  | "insert-target"
  | "insert-columns"
  | "insert-values"
  | "update-target"
  | "update-set-column"
  | "update-set-value"
  | "delete-target"
  | "join-condition"
  | "function-argument"
  | "schema-name"
  | "cte-reference"
  | "keyword"
  | "type"
  | "none";

export type SyntaxSnapshot = Readonly<{
  source: "raw" | "codemirror";
  rootName?: string;
  nodeName?: string;
  nodeFrom?: number;
  nodeTo?: number;
  errorTolerant: boolean;
}>;

export type ExpectedType = Readonly<{
  type: import("../catalog/types").SqlTypeRef;
  reason:
    | "comparison"
    | "assignment"
    | "function-argument"
    | "insert-value"
    | "unknown";
  confidence: number;
}>;

export type CompletionContext = Readonly<{
  kind: CompletionContextKind;
  range: TextRange;
  prefix: string;
  qualifier?: string;
  scope: Scope;
  graph: ScopeGraph;
  expectedType?: ExpectedType;
  operatorExpected: boolean;
  syntax?: SyntaxSnapshot;
}>;

export type CompletionRequest = Readonly<{
  document: SqlDocument;
  catalog: CatalogIndex;
  position: number;
  executionContext?: CompletionExecutionContext;
  signal?: AbortSignal;
  syntax?: SyntaxSnapshot;
  tokens?: readonly SqlToken[];
  maxItems?: number;
}>;

export type CompletionExecutionContext = Readonly<{
  catalog?: string;
  database?: string;
  schema?: string;
  service?: string;
  container?: string;
  user?: string;
}>;

export type CompletionResponse = Readonly<{
  documentId: string;
  documentVersion: number;
  catalogVersion: string;
  context: CompletionContext;
  items: readonly CompletionItem[];
  isIncomplete: boolean;
}>;

export type CompletionDependencies = Readonly<{
  dialect: SqlDialect;
}>;
