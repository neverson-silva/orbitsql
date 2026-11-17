import type { TextRange } from "../document/types";

export type SemanticIssueSeverity = "error" | "warning" | "info";

export type SemanticIssue = Readonly<{
  code: "unknown-relation" | "unresolved-qualifier" | "empty-statement";
  message: string;
  severity: SemanticIssueSeverity;
  range: TextRange;
}>;
