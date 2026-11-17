import { type CatalogIndex, identifierKey } from "../catalog";
import type {
  CompletionContext,
  CompletionContextKind,
  CompletionExecutionContext,
  SyntaxSnapshot,
} from "../completion/types";
import { getDialect, normalizeIdentifier } from "../dialect/registry";
import type { SqlDocument, TextRange } from "../document/types";
import {
  isWordToken,
  qualifiedNameFromTokens,
  type SqlToken,
  tokenizeSql,
  tokenText,
} from "../parser/tokens";
import { buildScopeGraph } from "./scope-graph";
import { inferExpectedType } from "./type-inference";
import type { Scope } from "./types";

type CursorFragment = Readonly<{
  range: TextRange;
  prefix: string;
  qualifier?: string;
}>;

type ClauseState = Readonly<{
  lastSelect: number;
  lastFrom: number;
  lastJoin: number;
  lastOn: number;
  lastWhere: number;
  lastHaving: number;
  lastGroup: number;
  lastOrder: number;
  lastInsert: number;
  lastInto: number;
  lastValues: number;
  lastUpdate: number;
  lastSet: number;
  lastDelete: number;
}>;

const relationModifiers = new Set([
  "left",
  "right",
  "full",
  "inner",
  "outer",
  "cross",
  "natural",
  "lateral",
]);

const relationBoundaryWords = new Set([
  ...relationModifiers,
  "on",
  "where",
  "group",
  "order",
  "having",
  "limit",
  "offset",
  "union",
  "returning",
  "qualify",
  "window",
]);

export const selectListContinuationWords = [
  "from",
  "into",
  "where",
  "group",
  "having",
  "order",
  "union",
  "intersect",
  "except",
  "limit",
  "offset",
  "fetch",
  "for",
  "option",
  "returning",
  "output",
] as const;

function lastTokenIndexBefore(
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

function relationAliasAtCursor(
  tokens: readonly SqlToken[],
  position: number,
  state: ClauseState,
  fragment: CursorFragment,
): "alias" | "column" | undefined {
  const relationIndex = Math.max(state.lastFrom, state.lastJoin);
  if (relationIndex < 0) return undefined;
  let sourceIndex = relationIndex + 1;
  while (relationModifiers.has(tokenText(tokens[sourceIndex])))
    sourceIndex += 1;
  const cursorIndex = lastTokenIndexBefore(tokens, position);
  if (cursorIndex < sourceIndex) return undefined;
  let depth = 0;
  for (let index = sourceIndex; index <= cursorIndex; index += 1) {
    if (tokens[index]?.text === "(") depth += 1;
    if (tokens[index]?.text === ")") depth = Math.max(0, depth - 1);
    if (depth === 0 && tokens[index]?.text === ",") {
      sourceIndex = index + 1;
      while (relationModifiers.has(tokenText(tokens[sourceIndex])))
        sourceIndex += 1;
    }
  }
  if (tokens[sourceIndex]?.text === "(") return undefined;
  const qualified = qualifiedNameFromTokens(tokens, sourceIndex, cursorIndex);
  if (!qualified.name.object.normalized) return undefined;
  let aliasIndex = qualified.next;
  if (tokenText(tokens[aliasIndex]) === "as") {
    if (aliasIndex === cursorIndex) return "alias";
    aliasIndex += 1;
  }
  const alias = tokens[aliasIndex];
  if (
    fragment.qualifier &&
    alias &&
    isWordToken(alias) &&
    identifierKey(alias.normalized) === identifierKey(fragment.qualifier) &&
    tokens[aliasIndex + 1]?.text === "."
  )
    return "column";
  if (!alias || !isWordToken(alias)) {
    const object = tokens[qualified.next - 1];
    return cursorIndex === qualified.next - 1 &&
      object !== undefined &&
      position > object.to
      ? "alias"
      : undefined;
  }
  if (relationBoundaryWords.has(tokenText(alias))) return undefined;
  return alias.from <= position && aliasIndex === cursorIndex
    ? "alias"
    : undefined;
}

function cursorFragment(
  text: string,
  position: number,
  dialect: ReturnType<typeof getDialect>,
): CursorFragment {
  const before = text.slice(0, position);
  const dot = before.lastIndexOf(".");
  if (dot >= 0) {
    const left = before.slice(0, dot).trimEnd();
    const leftStart = left.length - 1;
    const qualifier = readIdentifierBackward(left, leftStart, dialect);
    if (qualifier) {
      const rawRight = before.slice(dot + 1);
      const quotedStart = rawRight[0];
      if (quotedStart === '"' || quotedStart === "`" || quotedStart === "[") {
        const close = quotedStart === "[" ? "]" : quotedStart;
        if (!rawRight.slice(1).includes(close))
          return {
            qualifier: qualifier.value,
            prefix: rawRight.slice(1),
            range: {
              from:
                quotedStart === "[" && dialect.quoting.style === "bracket"
                  ? dot + 1
                  : dot + 2,
              to: position,
            },
          };
      }
      if (/^[\p{L}\p{N}_$]*$/u.test(rawRight)) {
        return {
          qualifier: qualifier.value,
          prefix: rawRight,
          range: { from: dot + 1, to: position },
        };
      }
      if (rawRight.length === 0) {
        return {
          qualifier: qualifier.value,
          prefix: "",
          range: { from: position, to: position },
        };
      }
    }
  }
  let start = position;
  while (start > 0 && /[\p{L}\p{N}_$]/u.test(text[start - 1])) start -= 1;
  if (
    start > 0 &&
    text[start - 1] === "[" &&
    dialect.quoting.style === "bracket"
  ) {
    return {
      prefix: text.slice(start, position),
      range: { from: start - 1, to: position },
    };
  }
  return {
    prefix: text.slice(start, position),
    range: { from: start, to: position },
  };
}

function readIdentifierBackward(
  text: string,
  end: number,
  dialect: ReturnType<typeof getDialect>,
): { value: string; from: number } | undefined {
  if (end < 0) return undefined;
  const current = text[end];
  if (current === '"' || current === "`" || current === "]") {
    const open = current === "]" ? "[" : current;
    const start = text.lastIndexOf(open, current === "]" ? end : end - 1);
    if (start < 0) return undefined;
    return {
      value: normalizeIdentifier(text.slice(start, end + 1), dialect),
      from: start,
    };
  }
  let start = end + 1;
  while (start > 0 && /[\p{L}\p{N}_$]/u.test(text[start - 1])) start -= 1;
  if (start === end + 1) return undefined;
  return {
    value: normalizeIdentifier(text.slice(start, end + 1), dialect),
    from: start,
  };
}

function statementBounds(
  tokens: readonly SqlToken[],
  position: number,
): { start: number; end: number } {
  let start = 0;
  let end = tokens.length - 1;
  for (let index = 0; index < tokens.length; index += 1) {
    if (tokens[index].text === ";" && tokens[index].to <= position)
      start = index + 1;
    if (tokens[index].text === ";" && tokens[index].from >= position) {
      end = index - 1;
      break;
    }
  }
  return { start, end };
}

function clauseState(
  tokens: readonly SqlToken[],
  start: number,
  position: number,
): ClauseState {
  const state = {
    lastSelect: -1,
    lastFrom: -1,
    lastJoin: -1,
    lastOn: -1,
    lastWhere: -1,
    lastHaving: -1,
    lastGroup: -1,
    lastOrder: -1,
    lastInsert: -1,
    lastInto: -1,
    lastValues: -1,
    lastUpdate: -1,
    lastSet: -1,
    lastDelete: -1,
  };
  const rootState = { ...state };
  const mark = (key: keyof ClauseState, index: number) => {
    if (depth === 0) rootState[key] = index;
    if (depth === cursorDepth) state[key] = index;
  };
  let depth = 0;
  for (const token of tokens) {
    if (token.from >= position) break;
    if (token.text === "(") depth += 1;
    if (token.text === ")") depth = Math.max(0, depth - 1);
  }
  const cursorDepth = depth;
  depth = 0;
  for (let index = start; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.from >= position || token.text === ";") break;
    if (token.text === "(") {
      depth += 1;
      continue;
    }
    if (token.text === ")") {
      depth = Math.max(0, depth - 1);
      continue;
    }
    const word = tokenText(token);
    if (word === "select") mark("lastSelect", index);
    else if (word === "from") mark("lastFrom", index);
    else if (word === "join") mark("lastJoin", index);
    else if (word === "on") mark("lastOn", index);
    else if (word === "where") mark("lastWhere", index);
    else if (word === "having") mark("lastHaving", index);
    else if (word === "group") mark("lastGroup", index);
    else if (word === "order") mark("lastOrder", index);
    else if (word === "insert") mark("lastInsert", index);
    else if (word === "into") mark("lastInto", index);
    else if (word === "values") mark("lastValues", index);
    else if (word === "update") mark("lastUpdate", index);
    else if (word === "set") mark("lastSet", index);
    else if (word === "delete") mark("lastDelete", index);
  }
  if (
    state.lastSelect < 0 &&
    state.lastInsert < 0 &&
    state.lastUpdate < 0 &&
    state.lastDelete < 0
  )
    return rootState;
  return state;
}

function isAfter(value: number, ...others: readonly number[]): boolean {
  return value >= 0 && others.every((other) => value > other);
}

function selectAliasAtCursor(
  tokens: readonly SqlToken[],
  position: number,
  state: ClauseState,
): boolean {
  if (state.lastSelect < 0) return false;
  const cursorIndex = lastTokenIndexBefore(tokens, position);
  let depth = 0;
  let aliasIndex = -1;
  for (let index = state.lastSelect + 1; index <= cursorIndex; index += 1) {
    const token = tokens[index];
    if (token.text === "(") {
      depth += 1;
      continue;
    }
    if (token.text === ")") {
      depth = Math.max(0, depth - 1);
      continue;
    }
    if (depth === 0 && tokenText(token) === "from") return false;
    if (depth === 0 && tokenText(token) === "as") aliasIndex = index;
  }
  if (aliasIndex < 0) return false;
  if (aliasIndex === cursorIndex) return true;
  const alias = tokens[aliasIndex + 1];
  return (
    alias !== undefined && isWordToken(alias) && aliasIndex + 1 === cursorIndex
  );
}

function functionArgumentIndex(
  tokens: readonly SqlToken[],
  position: number,
  dialect: ReturnType<typeof getDialect>,
): { functionName: string; argumentIndex: number } | undefined {
  let cursor = -1;
  for (let index = 0; index < tokens.length; index += 1) {
    if (tokens[index].to <= position) cursor = index;
    else break;
  }
  let nested = 0;
  for (let index = cursor; index >= 0; index -= 1) {
    const token = tokens[index];
    if (token.text === ")") {
      nested += 1;
      continue;
    }
    if (token.text !== "(") continue;
    if (nested > 0) {
      nested -= 1;
      continue;
    }
    const functionToken = tokens[index - 1];
    if (!functionToken || !isWordToken(functionToken)) return undefined;
    const functionName = tokenText(functionToken);
    let precedingIndex = index - 2;
    if (tokens[precedingIndex]?.text === ".") {
      while (
        precedingIndex >= 0 &&
        (tokens[precedingIndex].text === "." ||
          isWordToken(tokens[precedingIndex]))
      ) {
        if (
          ["from", "into", "join", "update", "set", "values"].includes(
            tokenText(tokens[precedingIndex]),
          )
        )
          break;
        precedingIndex -= 1;
      }
    }
    const preceding = tokenText(tokens[precedingIndex]);
    if (
      functionToken.kind === "identifier" &&
      ["from", "into", "join", "update", "set", "values"].includes(preceding)
    )
      return undefined;
    if (
      functionToken.kind === "keyword" &&
      !dialect.functions.has(functionToken.normalized.toUpperCase())
    )
      return undefined;
    let depth = 0;
    let argumentIndex = 0;
    for (let argument = index + 1; argument <= cursor; argument += 1) {
      if (tokens[argument].text === "(") depth += 1;
      if (tokens[argument].text === ")") depth = Math.max(0, depth - 1);
      if (tokens[argument].text === "," && depth === 0) argumentIndex += 1;
    }
    return { functionName, argumentIndex };
  }
  return undefined;
}

function selectListIsActive(state: ClauseState): boolean {
  return isAfter(
    state.lastSelect,
    state.lastFrom,
    state.lastJoin,
    state.lastOn,
    state.lastWhere,
    state.lastHaving,
    state.lastGroup,
    state.lastOrder,
    state.lastInto,
    state.lastValues,
    state.lastUpdate,
    state.lastSet,
    state.lastDelete,
  );
}

function selectListItemTokens(
  tokens: readonly SqlToken[],
  state: ClauseState,
  position: number,
): readonly SqlToken[] {
  let depth = 0;
  let current: SqlToken[] = [];
  for (let index = state.lastSelect + 1; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.from >= position || token.text === ";") break;
    if (token.text === "(") {
      depth += 1;
      current.push(token);
      continue;
    }
    if (token.text === ")") {
      depth = Math.max(0, depth - 1);
      current.push(token);
      continue;
    }
    if (depth === 0 && token.text === ",") {
      current = [];
      continue;
    }
    current.push(token);
  }
  return current;
}

function isSelectContinuationPrefix(
  value: string,
  dialect: ReturnType<typeof getDialect>,
): boolean {
  const normalized = value.toLocaleLowerCase();
  return selectListContinuationWords.some(
    (word) => dialect.keywords.has(word) && word.startsWith(normalized),
  );
}

function selectListContinuationAtCursor(
  tokens: readonly SqlToken[],
  position: number,
  state: ClauseState,
  dialect: ReturnType<typeof getDialect>,
): boolean {
  if (!selectListIsActive(state)) return false;
  const current = selectListItemTokens(tokens, state, position);
  if (current.length === 1 && current[0]?.text === "*") return true;
  const continuation = current[1];
  if (
    current.length === 2 &&
    continuation?.to === position &&
    selectListContinuationWords.some((word) => word === tokenText(continuation))
  )
    return false;
  return (
    current.length === 2 &&
    current[0]?.text === "*" &&
    isWordToken(continuation) &&
    isSelectContinuationPrefix(tokenText(continuation), dialect)
  );
}

function contextKind(
  tokens: readonly SqlToken[],
  position: number,
  fragment: CursorFragment,
  bounds: { start: number; end: number },
  dialect: ReturnType<typeof getDialect>,
  syntax?: SyntaxSnapshot,
): CompletionContextKind {
  if (
    syntax?.source === "codemirror" &&
    (syntax.nodeName === "String" ||
      syntax.nodeName === "LineComment" ||
      syntax.nodeName === "BlockComment")
  )
    return "none";
  const state = clauseState(tokens, bounds.start, position);
  const last = tokens
    .filter(
      (token) =>
        token.to <= position && token.from >= (tokens[bounds.start]?.from ?? 0),
    )
    .map(tokenText);
  const previous = last[last.length - 1] ?? "";
  const beforePrevious = last[last.length - 2] ?? "";
  let previousIndex = -1;
  for (let index = 0; index < tokens.length; index += 1) {
    if (tokens[index].to <= position) previousIndex = index;
    else break;
  }
  const previousToken = previousIndex >= 0 ? tokens[previousIndex] : undefined;
  void previousToken;
  const relationContext = relationAliasAtCursor(
    tokens,
    position,
    state,
    fragment,
  );
  if (relationContext === "alias") return "relation-alias";
  if (relationContext === "column") return "qualified-column";
  if (selectAliasAtCursor(tokens, position, state)) return "select-alias";
  if (functionArgumentIndex(tokens, position, dialect))
    return "function-argument";
  if (selectListContinuationAtCursor(tokens, position, state, dialect))
    return "select-list-continuation";
  if (beforePrevious === "schema") return "schema-name";
  if (isAfter(state.lastOn, state.lastJoin, state.lastWhere, state.lastHaving))
    return "join-condition";
  if (previous === "as" || beforePrevious === "as") return "type";
  if (isAfter(state.lastValues, state.lastInsert)) return "insert-values";
  if (isAfter(state.lastSet, state.lastUpdate, state.lastWhere)) {
    return previous === "=" || previous === ","
      ? "update-set-value"
      : "update-set-column";
  }
  if (isAfter(state.lastInto, state.lastInsert) && state.lastInsert >= 0) {
    if (
      tokens
        .slice(state.lastInto + 1, bounds.end + 1)
        .some((token) => token.text === "(")
    )
      return "insert-columns";
    return "insert-target";
  }
  if (
    isAfter(
      state.lastUpdate,
      state.lastSelect,
      state.lastInsert,
      state.lastDelete,
    )
  )
    return "update-target";
  if (isAfter(state.lastDelete, state.lastSelect, state.lastInsert))
    return "delete-target";
  if (
    isAfter(
      state.lastOrder,
      state.lastSelect,
      state.lastFrom,
      state.lastJoin,
      state.lastWhere,
      state.lastGroup,
      state.lastHaving,
    )
  )
    return "order-by-expression";
  if (
    isAfter(
      state.lastGroup,
      state.lastSelect,
      state.lastFrom,
      state.lastJoin,
      state.lastWhere,
      state.lastHaving,
      state.lastOrder,
    )
  )
    return "group-by-expression";
  if (
    isAfter(
      state.lastHaving,
      state.lastSelect,
      state.lastFrom,
      state.lastJoin,
      state.lastWhere,
      state.lastGroup,
      state.lastOrder,
    )
  )
    return "having-expression";
  if (
    isAfter(
      state.lastWhere,
      state.lastSelect,
      state.lastFrom,
      state.lastJoin,
      state.lastGroup,
      state.lastHaving,
      state.lastOrder,
    )
  )
    return "where-expression";
  if (
    isAfter(
      state.lastJoin,
      state.lastFrom,
      state.lastWhere,
      state.lastHaving,
      state.lastGroup,
      state.lastOrder,
    )
  )
    return "join-relation";
  if (
    isAfter(
      state.lastFrom,
      state.lastSelect,
      state.lastWhere,
      state.lastHaving,
      state.lastGroup,
      state.lastOrder,
      state.lastJoin,
    )
  )
    return "from-relation";
  if (fragment.qualifier) return "qualified-column";
  if (last.length === 0) return "statement-start";
  if (state.lastSelect >= 0) return "select-projection";
  return "keyword";
}

export function buildCompletionContext(
  document: SqlDocument,
  catalog: CatalogIndex,
  position: number,
  syntax?: SyntaxSnapshot,
  providedTokens?: readonly SqlToken[],
  executionContext?: CompletionExecutionContext,
): CompletionContext {
  const safePosition = Math.max(0, Math.min(position, document.text.length));
  const dialect = getDialect(document.dialect);
  const tokens = providedTokens ?? tokenizeSql(document.text, dialect);
  return buildCompletionContextFromTokens(
    document,
    catalog,
    safePosition,
    tokens,
    syntax,
    executionContext,
  );
}

export function buildCompletionContextFromTokens(
  document: SqlDocument,
  catalog: CatalogIndex,
  position: number,
  tokens: readonly SqlToken[],
  syntax?: SyntaxSnapshot,
  executionContext?: CompletionExecutionContext,
): CompletionContext {
  const dialect = getDialect(document.dialect);
  const bounds = statementBounds(tokens, position);
  const fragment = cursorFragment(document.text, position, dialect);
  const graph = buildScopeGraph(
    document,
    tokens,
    catalog,
    position,
    executionContext,
  );
  const scope: Scope =
    graph.scopes.find((candidate) => candidate.id === graph.currentScopeId) ??
    graph.scopes[0];
  const kind = contextKind(tokens, position, fragment, bounds, dialect, syntax);
  const relationKinds = [
    "from-relation",
    "join-relation",
    "insert-target",
    "update-target",
    "delete-target",
  ] as const;
  const lastToken = tokens[lastTokenIndexBefore(tokens, position)];
  const completedRelationKeyword =
    relationKinds.includes(kind as (typeof relationKinds)[number]) &&
    lastToken?.to === position &&
    ["from", "join", "into", "update", "delete"].includes(tokenText(lastToken));
  const completionFragment = completedRelationKeyword
    ? {
        prefix: "",
        qualifier: undefined,
        range: { from: position, to: position },
      }
    : fragment;
  const context: CompletionContext = {
    kind,
    qualifier: completionFragment.qualifier,
    prefix: completionFragment.prefix,
    range: completionFragment.range,
    scope,
    graph,
    operatorExpected:
      !fragment.qualifier &&
      ["where-expression", "having-expression", "join-condition"].includes(
        kind,
      ) &&
      [
        "identifier",
        "quoted-identifier",
        "number",
        "string",
        "parameter",
      ].includes(
        tokens
          .slice()
          .reverse()
          .find((token) => token.to <= position)?.kind ?? "",
      ),
    syntax: syntax ?? { source: "raw", errorTolerant: true },
  };
  return {
    ...context,
    expectedType: inferExpectedType(document, catalog, context, tokens),
  };
}
