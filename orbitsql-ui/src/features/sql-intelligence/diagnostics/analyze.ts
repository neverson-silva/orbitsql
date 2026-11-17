import { type CatalogIndex, findObject } from "../catalog";
import { getDialect } from "../dialect/registry";
import type { SqlDocument } from "../document/types";
import {
  qualifiedNameFromTokens,
  tokenizeSql,
  tokenText,
} from "../parser/tokens";
import { buildCompletionContext } from "../semantic/context";
import { resolveBinding } from "../semantic/scope-graph";
import type { SemanticIssue } from "./types";

const relationWords = new Set(["from", "join", "update", "into"]);

export function analyzeSql(
  document: SqlDocument,
  catalog: CatalogIndex,
): readonly SemanticIssue[] {
  const tokens = tokenizeSql(document.text, getDialect(document.dialect));
  const issues: SemanticIssue[] = [];
  const cteNames = new Set<string>();
  for (let index = 0; index < tokens.length - 1; index += 1) {
    if (tokenText(tokens[index]) !== "with") continue;
    const name = tokens[index + 1];
    if (name && name.kind !== "punctuation")
      cteNames.add(name.normalized.toLowerCase());
  }
  for (let index = 0; index < tokens.length; index += 1) {
    if (!relationWords.has(tokenText(tokens[index]))) continue;
    const candidate = tokens[index + 1];
    if (!candidate || candidate.kind === "punctuation") continue;
    if (
      candidate.text === "(" ||
      cteNames.has(candidate.normalized.toLowerCase())
    )
      continue;
    const qualified = qualifiedNameFromTokens(
      tokens,
      index + 1,
      tokens.length - 1,
    );
    if (!qualified.name.object.normalized) continue;
    if (!findObject(catalog, qualified.name)) {
      issues.push({
        code: "unknown-relation",
        message: `Relation ${qualified.name.object.normalized} was not found in the catalog`,
        severity: "warning",
        range: {
          from: candidate.from,
          to: tokens[qualified.next - 1]?.to ?? candidate.to,
        },
      });
    }
  }
  const context = buildCompletionContext(
    document,
    catalog,
    document.text.length,
  );
  for (let index = 0; index < tokens.length - 2; index += 1) {
    const qualifier = tokens[index];
    if (
      !qualifier ||
      (qualifier.kind !== "identifier" &&
        qualifier.kind !== "quoted-identifier") ||
      tokens[index + 1]?.text !== "." ||
      !tokens[index + 2]
    )
      continue;
    const name = qualifier.normalized.toLocaleLowerCase();
    if (
      catalog.schemasByName.has(name) ||
      resolveBinding(context.scope, context.graph.scopes, qualifier.normalized)
    )
      continue;
    issues.push({
      code: "unresolved-qualifier",
      message: `Qualifier ${qualifier.normalized} is not visible in this scope`,
      severity: "warning",
      range: { from: qualifier.from, to: qualifier.to },
    });
  }
  if (document.text.trim().length === 0) {
    issues.push({
      code: "empty-statement",
      message: "The SQL document is empty",
      severity: "info",
      range: { from: 0, to: 0 },
    });
  }
  return issues;
}
