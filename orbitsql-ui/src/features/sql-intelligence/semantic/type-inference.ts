import { type CatalogIndex, findObject, type SqlTypeRef } from "../catalog";
import type { CompletionContext, ExpectedType } from "../completion/types";
import { getDialect } from "../dialect/registry";
import type { SqlDocument } from "../document/types";
import {
  isWordToken,
  qualifiedNameFromTokens,
  type SqlToken,
  tokenizeSql,
  tokenText,
} from "../parser/tokens";
import { resolveBinding } from "./scope-graph";
import { type Binding, bindingColumn } from "./types";

function typeFromFamily(family: SqlTypeRef["family"]): SqlTypeRef {
  return { family, name: family };
}

function columnType(
  name: string,
  context: CompletionContext,
): SqlTypeRef | undefined {
  const parts = name.split(".").filter(Boolean);
  const columnName = parts[parts.length - 1];
  const bindingName = parts.length > 1 ? parts[parts.length - 2] : undefined;
  const bindings = bindingName
    ? [resolveBinding(context.scope, context.graph.scopes, bindingName)].filter(
        (binding): binding is Binding => binding !== undefined,
      )
    : context.scope.bindings;
  for (const binding of bindings) {
    const column = bindingColumn(binding, columnName);
    if (column) return column.type;
  }
  return undefined;
}

function trailingQualifiedName(
  tokens: readonly SqlToken[],
): string | undefined {
  const parts: string[] = [];
  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    const token = tokens[index];
    if (isWordToken(token)) {
      parts.unshift(token.normalized);
      continue;
    }
    if (token.text === ".") continue;
    break;
  }
  return parts.length > 0 ? parts.join(".") : undefined;
}

function assignmentTypeFromTokens(
  before: string,
  context: CompletionContext,
  catalog: CatalogIndex,
  document: SqlDocument,
  providedTokens?: readonly SqlToken[],
): SqlTypeRef | undefined {
  const tokens = providedTokens
    ? providedTokens.filter((token) => token.to <= before.length)
    : tokenizeSql(before, getDialect(document.dialect));
  if (tokenText(tokens[tokens.length - 1]) !== "=") return undefined;
  const name = trailingQualifiedName(tokens.slice(0, -1));
  if (!name) return undefined;
  const direct = columnType(name, context);
  if (direct) return direct;
  let updateIndex = -1;
  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    if (tokenText(tokens[index]) === "update") {
      updateIndex = index;
      break;
    }
  }
  if (updateIndex < 0) return undefined;
  const updateTarget = qualifiedNameFromTokens(
    tokens,
    updateIndex + 1,
    tokens.length - 1,
  );
  const object = updateTarget.name.object.normalized
    ? findObject(catalog, updateTarget.name)
    : undefined;
  const nameParts = name.split(".");
  const columnName = nameParts[nameParts.length - 1]?.toLocaleLowerCase();
  return object?.columns.find(
    (column) => column.name.normalized.toLocaleLowerCase() === columnName,
  )?.type;
}

function expectedFromAssignment(
  before: string,
  context: CompletionContext,
  catalog: CatalogIndex,
  document: SqlDocument,
  providedTokens?: readonly SqlToken[],
): ExpectedType | undefined {
  const match = before.match(
    /(?:^|[,;]\s*|\bset\s+)([\p{L}\p{N}_$]+(?:\.[\p{L}\p{N}_$]+)?)\s*=\s*$/iu,
  );
  if (!match) {
    const type = assignmentTypeFromTokens(
      before,
      context,
      catalog,
      document,
      providedTokens,
    );
    return type ? { type, reason: "assignment", confidence: 0.92 } : undefined;
  }
  let type = columnType(match[1], context);
  if (!type) {
    type = assignmentTypeFromTokens(
      before,
      context,
      catalog,
      document,
      providedTokens,
    );
  }
  return type ? { type, reason: "assignment", confidence: 0.96 } : undefined;
}

function expectedFromComparison(
  before: string,
  context: CompletionContext,
  document: SqlDocument,
  providedTokens?: readonly SqlToken[],
): ExpectedType | undefined {
  const match = before.match(
    /([\p{L}\p{N}_$]+(?:\.[\p{L}\p{N}_$]+)?)\s*(?:=|<>|!=|>=|<=|>|<|like|ilike|in|is)\s*$/iu,
  );
  if (!match) {
    const tokens = providedTokens
      ? providedTokens.filter((token) => token.to <= before.length)
      : tokenizeSql(before, getDialect(document.dialect));
    const operator = tokenText(tokens[tokens.length - 1]);
    if (
      ![
        "=",
        "<>",
        "!=",
        ">=",
        "<=",
        ">",
        "<",
        "like",
        "ilike",
        "in",
        "is",
      ].includes(operator)
    )
      return undefined;
    const name = trailingQualifiedName(tokens.slice(0, -1));
    const type = name ? columnType(name, context) : undefined;
    return type ? { type, reason: "comparison", confidence: 0.9 } : undefined;
  }
  const type = columnType(match[1], context);
  return type ? { type, reason: "comparison", confidence: 0.94 } : undefined;
}

function expectedFromFunction(
  before: string,
  document: SqlDocument,
  catalog: CatalogIndex,
  providedTokens?: readonly SqlToken[],
): ExpectedType | undefined {
  const dialect = getDialect(document.dialect);
  const tokens = providedTokens
    ? providedTokens.filter((token) => token.to <= before.length)
    : tokenizeSql(before, dialect);
  let depth = 0;
  let openIndex = -1;
  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    if (tokens[index].text === ")") depth += 1;
    else if (tokens[index].text === "(") {
      if (depth === 0) {
        openIndex = index;
        break;
      }
      depth -= 1;
    }
  }
  const functionToken = openIndex > 0 ? tokens[openIndex - 1] : undefined;
  if (!functionToken || !isWordToken(functionToken)) return undefined;
  let nested = 0;
  let argumentIndex = 0;
  for (let index = openIndex + 1; index < tokens.length; index += 1) {
    if (tokens[index].text === "(") nested += 1;
    if (tokens[index].text === ")") nested = Math.max(0, nested - 1);
    if (tokens[index].text === "," && nested === 0) argumentIndex += 1;
  }
  const functionName = functionToken.normalized.toLocaleUpperCase();
  const definition = dialect.functions.get(functionName);
  const catalogFunction = catalog.functionsByName.get(
    functionToken.normalized.toLocaleLowerCase(),
  )?.[0];
  const family =
    catalogFunction?.parameters[argumentIndex]?.type.family ??
    definition?.argumentFamilies[argumentIndex];
  return family
    ? {
        type: typeFromFamily(family),
        reason: "function-argument",
        confidence: 0.78,
      }
    : undefined;
}

function expectedFromInsertValues(
  before: string,
  catalog: CatalogIndex,
  document: SqlDocument,
  providedTokens?: readonly SqlToken[],
): ExpectedType | undefined {
  const tokens = providedTokens
    ? providedTokens.filter((token) => token.to <= before.length)
    : tokenizeSql(before, getDialect(document.dialect));
  let insertIndex = -1;
  let valuesIndex = -1;
  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    if (valuesIndex < 0 && tokenText(tokens[index]) === "values")
      valuesIndex = index;
    if (tokenText(tokens[index]) === "insert") {
      insertIndex = index;
      break;
    }
  }
  if (insertIndex < 0 || valuesIndex < 0 || valuesIndex < insertIndex)
    return undefined;
  const intoIndex = tokens.findIndex(
    (token, index) => index > insertIndex && tokenText(token) === "into",
  );
  const qualifiedTarget = qualifiedNameFromTokens(
    tokens,
    intoIndex + 1,
    valuesIndex - 1,
  );
  const object = qualifiedTarget.name.object.normalized
    ? findObject(catalog, qualifiedTarget.name)
    : undefined;
  if (!object) return undefined;
  const columnsOpen = tokens.findIndex(
    (token, index) => index > intoIndex && token.text === "(",
  );
  const columnsClose =
    columnsOpen >= 0
      ? tokens.findIndex(
          (token, index) => index > columnsOpen && token.text === ")",
        )
      : -1;
  if (
    columnsOpen < 0 ||
    columnsClose < 0 ||
    columnsClose > valuesIndex ||
    tokens[valuesIndex + 1]?.text !== "("
  )
    return undefined;
  const columns = tokens
    .slice(columnsOpen + 1, columnsClose)
    .filter(isWordToken)
    .map((token) => token.normalized);
  let currentValueIndex = 0;
  let depth = 0;
  for (let index = valuesIndex + 2; index < tokens.length; index += 1) {
    if (tokens[index].text === "(") depth += 1;
    if (tokens[index].text === ")") depth = Math.max(0, depth - 1);
    if (tokens[index].text === "," && depth === 0) currentValueIndex += 1;
  }
  const columnTarget = object.columns.find(
    (column) =>
      column.name.normalized.toLocaleLowerCase() ===
      columns[currentValueIndex]?.toLocaleLowerCase(),
  );
  if (
    !columnTarget ||
    columnTarget.generated ||
    columnTarget.identity ||
    columnTarget.computed
  )
    return undefined;
  return { type: columnTarget.type, reason: "insert-value", confidence: 0.93 };
}

export function inferExpectedType(
  document: SqlDocument,
  catalog: CatalogIndex,
  context: CompletionContext,
  providedTokens?: readonly SqlToken[],
): ExpectedType | undefined {
  const before = document.text.slice(0, context.range.from);
  return (
    expectedFromAssignment(
      before,
      context,
      catalog,
      document,
      providedTokens,
    ) ??
    expectedFromInsertValues(before, catalog, document, providedTokens) ??
    expectedFromComparison(before, context, document, providedTokens) ??
    expectedFromFunction(before, document, catalog, providedTokens)
  );
}
