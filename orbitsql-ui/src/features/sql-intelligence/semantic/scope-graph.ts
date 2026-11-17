import {
  type CatalogIndex,
  type CatalogObject,
  findObjectInExecutionContext,
  identifierKey,
  type SqlTypeRef,
} from "../catalog";
import type { CompletionExecutionContext } from "../completion/types";
import type { SqlDocument, TextRange } from "../document/types";
import {
  isWordToken,
  matchingParen,
  qualifiedNameFromTokens,
  type SqlToken,
  tokenText,
} from "../parser/tokens";
import {
  type Binding,
  type BindingKind,
  columnsFromObject,
  type ProjectionSymbol,
  type Scope,
  type ScopeGraph,
} from "./types";

const unknownType: SqlTypeRef = { family: "unknown", name: "unknown" };
const clauseWords = new Set([
  "on",
  "where",
  "join",
  "left",
  "right",
  "inner",
  "full",
  "cross",
  "group",
  "order",
  "having",
  "limit",
  "offset",
  "union",
  "returning",
]);

const sourceModifiers = new Set([
  "left",
  "right",
  "full",
  "inner",
  "outer",
  "cross",
  "natural",
  "lateral",
]);

function rangeFromTokens(
  tokens: readonly SqlToken[],
  start: number,
  end: number,
  fallback: TextRange,
): TextRange {
  return {
    from: tokens[start]?.from ?? fallback.from,
    to: tokens[end]?.to ?? fallback.to,
  };
}

function lastIndexBefore(
  tokens: readonly SqlToken[],
  position: number,
): number {
  let result = -1;
  for (let index = 0; index < tokens.length; index += 1) {
    if (tokens[index].to <= position) result = index;
    else break;
  }
  return result;
}

function findScope(scopes: readonly Scope[], position: number): Scope {
  return (
    scopes
      .filter(
        (scope) => scope.range.from <= position && position <= scope.range.to,
      )
      .sort((left, right) => right.range.from - left.range.from)[0] ?? scopes[0]
  );
}

function topLevelIndices(
  tokens: readonly SqlToken[],
  start: number,
  end: number,
): readonly number[] {
  const result: number[] = [];
  let depth = 0;
  for (let index = Math.max(0, start); index <= end; index += 1) {
    const token = tokens[index];
    if (!token) continue;
    if (token.text === "(") {
      depth += 1;
      continue;
    }
    if (token.text === ")") {
      depth = Math.max(0, depth - 1);
      continue;
    }
    if (depth === 0) result.push(index);
  }
  return result;
}

function readAlias(
  tokens: readonly SqlToken[],
  start: number,
): { name?: string; next: number } {
  let index = start;
  if (tokenText(tokens[index]) === "as") index += 1;
  const token = tokens[index];
  if (token && isWordToken(token) && !clauseWords.has(tokenText(token)))
    return { name: token.normalized, next: index + 1 };
  return { next: start };
}

function bindingForObject(
  name: string,
  object: CatalogObject | undefined,
  range: TextRange,
  kind?: BindingKind,
): Binding {
  const resolvedKind =
    kind ?? (object && "temporary" in object ? "table" : "view");
  return {
    name,
    normalizedName: name.toLocaleLowerCase(),
    kind: resolvedKind,
    range,
    object,
    columns: columnsFromObject(object),
    sourceName: object?.name.normalized,
  };
}

function findChildScope(
  scopes: readonly Scope[],
  range: TextRange,
): Scope | undefined {
  return scopes.find(
    (scope) => scope.range.from === range.from && scope.range.to === range.to,
  );
}

function subqueryRanges(
  tokens: readonly SqlToken[],
  start: number,
  end: number,
): readonly TextRange[] {
  const result: TextRange[] = [];
  for (let index = start; index <= end; index += 1) {
    if (tokens[index]?.text !== "(") continue;
    const next = tokenText(tokens[index + 1]);
    if (next !== "select" && next !== "with") continue;
    const close = matchingParen(tokens, index);
    if (close !== undefined && close <= end)
      result.push(rangeFromTokens(tokens, index, close, { from: 0, to: 0 }));
  }
  return result;
}

function collectCtes(
  tokens: readonly SqlToken[],
  start: number,
  end: number,
  scopes: readonly Scope[],
): readonly Binding[] {
  if (tokenText(tokens[start]) !== "with") return [];
  const result: Binding[] = [];
  let index = start + 1;
  while (index <= end && isWordToken(tokens[index])) {
    const name = tokens[index].normalized;
    index += 1;
    let explicitColumns: readonly string[] | undefined;
    if (tokens[index]?.text === "(") {
      const close = matchingParen(tokens, index);
      if (close === undefined) return result;
      explicitColumns = tokens
        .slice(index + 1, close)
        .filter(isWordToken)
        .map((token) => token.normalized);
      index = close + 1;
    }
    if (tokenText(tokens[index]) !== "as" || tokens[index + 1]?.text !== "(")
      return result;
    const open = index + 1;
    const close = matchingParen(tokens, open);
    if (close === undefined) return result;
    const range = rangeFromTokens(tokens, open, close, { from: 0, to: 0 });
    const child = findChildScope(scopes, range);
    const columns =
      child?.outputSymbols.map((symbol, columnIndex) => ({
        ...symbol,
        name: explicitColumns?.[columnIndex] ?? symbol.name,
      })) ?? [];
    result.push({
      name,
      normalizedName: identifierKey(name),
      kind: "cte",
      range,
      columns,
      sourceName: name,
    });
    index = close + 1;
    if (tokenText(tokens[index]) !== ",") break;
    index += 1;
  }
  return result;
}

function readSources(
  tokens: readonly SqlToken[],
  start: number,
  end: number,
  index: CatalogIndex,
  scopes: readonly Scope[],
  scope: Scope,
  ctes: readonly Binding[],
  executionContext?: CompletionExecutionContext,
): readonly Binding[] {
  const result: Binding[] = [];
  for (const tokenIndex of topLevelIndices(tokens, start, end)) {
    const word = tokenText(tokens[tokenIndex]);
    if (word !== "from" && word !== "join") continue;
    let cursor = tokenIndex + 1;
    while (
      [
        "left",
        "right",
        "full",
        "inner",
        "outer",
        "cross",
        "natural",
        "lateral",
      ].includes(tokenText(tokens[cursor]))
    )
      cursor += 1;
    while (cursor <= end) {
      if (tokens[cursor]?.text === "(") {
        const close = matchingParen(tokens, cursor);
        if (close === undefined) break;
        const range = rangeFromTokens(tokens, cursor, close, scope.range);
        const child = findChildScope(scopes, range);
        const aliasResult = readAlias(tokens, close + 1);
        if (aliasResult.name && child)
          result.push({
            name: aliasResult.name,
            normalizedName: aliasResult.name.toLocaleLowerCase(),
            kind: "derived",
            range,
            columns: child.outputSymbols,
            sourceName: "derived",
          });
        cursor = aliasResult.next > close + 1 ? aliasResult.next : close + 1;
      } else {
        const qualified = qualifiedNameFromTokens(tokens, cursor, end);
        if (!qualified.name.object.normalized) break;
        const objectName = qualified.name.object.normalized;
        const aliasResult = readAlias(tokens, qualified.next);
        const alias = aliasResult.name ?? objectName;
        const cte = ctes.find(
          (candidate) => candidate.normalizedName === identifierKey(objectName),
        );
        const sourceRange = rangeFromTokens(
          tokens,
          cursor,
          qualified.next - 1,
          scope.range,
        );
        if (cte) {
          result.push({
            ...cte,
            name: alias,
            normalizedName: alias.toLocaleLowerCase(),
            range: sourceRange,
          });
        } else {
          result.push(
            bindingForObject(
              alias,
              findObjectInExecutionContext(
                index,
                qualified.name,
                executionContext,
              ),
              sourceRange,
            ),
          );
        }
        cursor =
          aliasResult.next > qualified.next ? aliasResult.next : qualified.next;
      }
      if (tokens[cursor]?.text !== ",") break;
      cursor += 1;
      while (sourceModifiers.has(tokenText(tokens[cursor]))) cursor += 1;
    }
  }
  return result;
}

function projectedSymbols(
  tokens: readonly SqlToken[],
  start: number,
  end: number,
  bindings: readonly Binding[],
): readonly ProjectionSymbol[] {
  const topLevel = topLevelIndices(tokens, start, end);
  const selectIndex = topLevel.find(
    (index) => tokenText(tokens[index]) === "select",
  );
  if (selectIndex === undefined) return [];
  const fromIndex = topLevel.find(
    (index) => index > selectIndex && tokenText(tokens[index]) === "from",
  );
  const projectionEnd = fromIndex === undefined ? end : fromIndex - 1;
  const result: ProjectionSymbol[] = [];
  let expressionStart = selectIndex + 1;
  let depth = 0;
  const addExpression = (expressionEnd: number) => {
    const expression = tokens
      .slice(expressionStart, expressionEnd + 1)
      .filter(Boolean);
    const words = expression.filter(isWordToken);
    if (expression.length === 0 || words.length === 0) return;
    const asIndex = expression.findIndex((token) => tokenText(token) === "as");
    const alias =
      asIndex >= 0
        ? expression[asIndex + 1]?.normalized
        : words.length > 1
          ? words[words.length - 1].normalized
          : words[0].normalized;
    if (!alias || alias === "*") return;
    const sourceName =
      words.length > 1 && expression.some((token) => tokenText(token) === ".")
        ? words[0].normalized
        : undefined;
    const sourceColumnName = sourceName
      ? words[words.length - 1].normalized
      : words[0].normalized;
    const sourceBinding = sourceName
      ? bindings.find(
          (binding) =>
            binding.normalizedName === sourceName.toLocaleLowerCase(),
        )
      : bindings[0];
    const sourceColumn = sourceBinding?.columns.find(
      (column) =>
        column.name.toLocaleLowerCase() ===
        sourceColumnName.toLocaleLowerCase(),
    );
    result.push({
      name: alias,
      type: sourceColumn?.type ?? unknownType,
      sourceColumn: sourceColumn?.sourceColumn,
    });
  };
  for (let index = selectIndex + 1; index <= projectionEnd; index += 1) {
    if (tokens[index].text === "(") depth += 1;
    if (tokens[index].text === ")") depth = Math.max(0, depth - 1);
    if (tokens[index].text === "," && depth === 0) {
      addExpression(index - 1);
      expressionStart = index + 1;
    }
  }
  addExpression(projectionEnd);
  return result;
}

export function resolveBinding(
  scope: Scope,
  scopes: readonly Scope[],
  name: string,
): Binding | undefined {
  const normalized = name.toLocaleLowerCase();
  let current: Scope | undefined = scope;
  while (current) {
    const binding = [...current.bindings, ...current.ctes].find(
      (candidate) => candidate.normalizedName === normalized,
    );
    if (binding) return binding;
    current = current.parentId
      ? scopes.find((candidate) => candidate.id === current?.parentId)
      : undefined;
  }
  return undefined;
}

export function buildScopeGraph(
  document: SqlDocument,
  tokens: readonly SqlToken[],
  index: CatalogIndex,
  position: number,
  executionContext?: CompletionExecutionContext,
): ScopeGraph {
  const cursorToken = lastIndexBefore(tokens, position);
  let start = 0;
  for (let tokenIndex = cursorToken; tokenIndex >= 0; tokenIndex -= 1) {
    if (tokens[tokenIndex].text === ";") {
      start = tokenIndex + 1;
      break;
    }
  }
  let end = tokens.length - 1;
  for (let tokenIndex = start; tokenIndex < tokens.length; tokenIndex += 1) {
    if (tokens[tokenIndex].text === ";") {
      end = tokenIndex - 1;
      break;
    }
  }
  const fallbackRange = {
    from: tokens[start]?.from ?? 0,
    to: tokens[end]?.to ?? document.text.length,
  };
  const ranges = subqueryRanges(tokens, start, end);
  const scopes: Scope[] = [
    {
      id: "scope:root",
      range: fallbackRange,
      bindings: [],
      ctes: [],
      outputSymbols: [],
    },
  ];
  for (const [scopeIndex, range] of ranges.entries()) {
    let parent = scopes[0];
    for (const candidate of scopes) {
      if (
        candidate.range.from <= range.from &&
        candidate.range.to >= range.to &&
        candidate.range.from >= parent.range.from &&
        candidate.range.to <= parent.range.to
      )
        parent = candidate;
    }
    scopes.push({
      id: `scope:${scopeIndex + 1}`,
      parentId: parent.id,
      range,
      bindings: [],
      ctes: [],
      outputSymbols: [],
    });
  }
  scopes[0] = {
    ...scopes[0],
    ctes: collectCtes(tokens, start, end, scopes),
  };
  const processOrder = [...scopes.keys()].sort((left, right) =>
    left === 0 ? 1 : right === 0 ? -1 : left - right,
  );
  for (const scopeIndex of processOrder) {
    const scope = scopes[scopeIndex];
    let scopeStart = start;
    while (scopeStart <= end && tokens[scopeStart].from < scope.range.from)
      scopeStart += 1;
    let scopeEnd = end;
    while (scopeEnd >= start && tokens[scopeEnd].to > scope.range.to)
      scopeEnd -= 1;
    if (scopeIndex > 0 && tokens[scopeStart]?.text === "(") scopeStart += 1;
    if (scopeIndex > 0 && tokens[scopeEnd]?.text === ")") scopeEnd -= 1;
    const parent = scope.parentId
      ? scopes.find((candidate) => candidate.id === scope.parentId)
      : undefined;
    const visibleCtes = scope.parentId
      ? (parent?.ctes ?? scopes[0].ctes)
      : collectCtes(tokens, start, end, scopes);
    const bindings = readSources(
      tokens,
      scopeStart,
      scopeEnd,
      index,
      scopes,
      scope,
      visibleCtes,
      executionContext,
    );
    scopes[scopeIndex] = {
      ...scope,
      ctes: visibleCtes,
      bindings,
      outputSymbols: projectedSymbols(tokens, scopeStart, scopeEnd, bindings),
    };
  }
  const current = findScope(scopes, position);
  return { scopes, rootScopeId: "scope:root", currentScopeId: current.id };
}
