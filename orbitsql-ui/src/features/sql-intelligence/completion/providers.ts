import {
  type CatalogObject,
  type CatalogType,
  findObject,
  identifierKey,
  type SqlTypeRef,
} from "../catalog";
import {
  getDialect,
  isDialectKeyword,
  quoteIdentifier,
} from "../dialect/registry";
import type { SqlDocument } from "../document/types";
import {
  isWordToken,
  qualifiedNameFromTokens,
  type SqlToken,
  tokenizeSql,
  tokenText,
} from "../parser/tokens";
import { selectListContinuationWords } from "../semantic/context";
import { joinCandidates, type RelationshipGraph } from "../semantic/relations";
import { resolveBinding } from "../semantic/scope-graph";
import type { Binding, ProjectionSymbol, Scope } from "../semantic/types";
import type {
  CompletionContext,
  CompletionItem,
  CompletionKind,
  CompletionProvenance,
  CompletionRequest,
  RankingFeatures,
} from "./types";

const neutralFeatures: RankingFeatures = {
  prefixQuality: 0,
  semanticFit: 0,
  scopeDistance: 0,
  qualifierMatch: 0,
  typeCompatibility: 0.5,
  relationshipConfidence: 0,
  objectKindPrior: 0,
  schemaAffinity: 0,
  usageSignal: 0,
  ambiguityPenalty: 0,
};

function renderIdentifier(value: string, document: SqlDocument): string {
  const dialect = getDialect(document.dialect);
  return /^[\p{L}_$][\p{L}\p{N}_$]*$/u.test(value) &&
    !isDialectKeyword(value, dialect)
    ? value
    : quoteIdentifier(value, dialect);
}

function typeCompatibility(
  type: SqlTypeRef | undefined,
  expected: SqlTypeRef | undefined,
): number {
  if (!expected || expected.family === "unknown" || !type) return 0.5;
  if (type.family === expected.family) return 1;
  if (
    (type.family === "integer" ||
      type.family === "decimal" ||
      type.family === "float") &&
    (expected.family === "integer" ||
      expected.family === "decimal" ||
      expected.family === "float")
  )
    return 0.7;
  return 0.05;
}

function item(
  context: CompletionContext,
  _document: SqlDocument,
  label: string,
  insertText: string,
  kind: CompletionKind,
  score: number,
  detail?: string,
  sourceId?: string,
  type?: SqlTypeRef,
  provenance: CompletionProvenance = "catalog",
  features: Partial<RankingFeatures> = {},
): CompletionItem {
  const typeMatch = typeCompatibility(type, context.expectedType?.type);
  const mergedFeatures: RankingFeatures = {
    ...neutralFeatures,
    ...features,
    typeCompatibility: features.typeCompatibility ?? typeMatch,
  };
  return {
    from: context.range.from,
    to: context.range.to,
    insertText,
    displayLabel: label,
    kind,
    detail,
    score:
      score +
      mergedFeatures.typeCompatibility * 15 +
      mergedFeatures.semanticFit * 20,
    typeName: type?.name,
    sourceId,
    provenance,
    features: mergedFeatures,
  };
}

function visibleBindings(context: CompletionContext): readonly Binding[] {
  const result: Binding[] = [];
  let current: Scope | undefined = context.scope;
  while (current) {
    result.push(...current.bindings);
    current = current.parentId
      ? context.graph.scopes.find((scope) => scope.id === current?.parentId)
      : undefined;
  }
  return result;
}

function columnItem(
  context: CompletionContext,
  document: SqlDocument,
  binding: Binding,
  column: ProjectionSymbol,
): CompletionItem {
  const qualified = context.qualifier !== undefined;
  const label = qualified ? `${binding.name}.${column.name}` : column.name;
  const insertText = renderIdentifier(column.name, document);
  return item(
    context,
    document,
    label,
    insertText,
    "column",
    qualified ? 115 : 75,
    column.type.name,
    column.sourceColumn?.id,
    column.type,
    "catalog",
    {
      semanticFit: 1,
      qualifierMatch: qualified ? 1 : 0.5,
      scopeDistance:
        binding ===
        context.scope.bindings.find(
          (candidate) => candidate.normalizedName === binding.normalizedName,
        )
          ? 1
          : 0.6,
    },
  );
}

function matchesExpectedType(
  context: CompletionContext,
  type: SqlTypeRef | undefined,
): boolean {
  if (
    !context.expectedType ||
    context.expectedType.type.family === "unknown" ||
    !type
  )
    return true;
  return typeCompatibility(type, context.expectedType.type) >= 0.5;
}

function matchesPrefix(value: string, prefix: string): boolean {
  if (prefix.length === 0) return true;
  const normalizedValue = value.toLocaleLowerCase();
  const normalizedPrefix = prefix.toLocaleLowerCase();
  if (normalizedValue.startsWith(normalizedPrefix)) return true;
  let prefixIndex = 0;
  for (const character of normalizedValue) {
    if (character === normalizedPrefix[prefixIndex]) prefixIndex += 1;
    if (prefixIndex === normalizedPrefix.length) return true;
  }
  return false;
}

function requestTokens(request: CompletionRequest): readonly SqlToken[] {
  return (
    request.tokens ??
    tokenizeSql(request.document.text, getDialect(request.document.dialect))
  );
}

function matchesExecutionContext(
  request: CompletionRequest,
  object: CatalogObject,
): boolean {
  const executionContext = request.executionContext;
  if (!executionContext) return true;
  const schemaName = executionContext.schema ?? executionContext.user;
  const databaseName = executionContext.database ?? executionContext.catalog;
  const schema = schemaName
    ? request.catalog.snapshot.schemas.find(
        (item) => identifierKey(item.name) === identifierKey(schemaName),
      )
    : undefined;
  const database = databaseName
    ? request.catalog.snapshot.databases.find(
        (item) =>
          identifierKey(item.name) === identifierKey(databaseName) ||
          identifierKey(item.id) === identifierKey(databaseName),
      )
    : undefined;
  if (schema && object.schemaId !== schema.id) return false;
  if (database && object.databaseId !== database.id) return false;
  return true;
}

function enumLiteralValues(definition: string): readonly string[] {
  const body = definition.match(/^(?:enum|set)\s*\((.*)\)$/iu)?.[1];
  if (!body) return [];
  const values: string[] = [];
  const literalPattern = /'((?:''|[^'])*)'|"((?:""|[^"])*)"/gu;
  for (const match of body.matchAll(literalPattern)) {
    const value =
      match[1]?.replace(/''/g, "'") ?? match[2]?.replace(/""/g, '"');
    if (value !== undefined) values.push(value);
  }
  return values;
}

function enumValues(
  request: CompletionRequest,
  context: CompletionContext,
): readonly string[] {
  if (context.expectedType?.type.family !== "enum") return [];
  const names = new Set<string>([
    context.expectedType.type.name.toLocaleLowerCase(),
  ]);
  const values = new Set<string>();
  for (const type of request.catalog.snapshot.types) {
    if (type.family !== "enum") continue;
    if (names.has(type.name.normalized.toLocaleLowerCase()))
      for (const value of enumLiteralValues(
        type.definition ?? type.name.normalized,
      ))
        values.add(value);
  }
  for (const value of enumLiteralValues(context.expectedType.type.name))
    values.add(value);
  return [...values];
}

export function provideColumnCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const bindings = context.qualifier
    ? [
        resolveBinding(context.scope, context.graph.scopes, context.qualifier),
      ].filter((binding): binding is Binding => binding !== undefined)
    : visibleBindings(context);
  const result: CompletionItem[] = [];
  for (const binding of bindings) {
    for (const column of binding.columns) {
      if (!matchesPrefix(column.name, context.prefix)) continue;
      if (
        (context.kind === "function-argument" ||
          context.kind === "insert-values" ||
          context.kind === "update-set-value") &&
        !matchesExpectedType(context, column.type)
      )
        continue;
      result.push(columnItem(context, request.document, binding, column));
    }
  }
  return result;
}

function sourceObjectFromDocument(
  request: CompletionRequest,
  keyword: "insert" | "update",
): CatalogObject | undefined {
  const tokens = requestTokens(request).filter(
    (token) => token.to <= request.position,
  );
  let keywordIndex = -1;
  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    if (tokens[index].normalized.toLocaleLowerCase() === keyword) {
      keywordIndex = index;
      break;
    }
  }
  const objectOffset = keyword === "insert" ? 2 : 1;
  const qualified = qualifiedNameFromTokens(
    tokens,
    keywordIndex + objectOffset,
    tokens.length - 1,
  );
  return qualified.name.object.normalized
    ? findObject(request.catalog, qualified.name)
    : undefined;
}

function insertColumnNames(
  request: CompletionRequest,
  keyword: "insert" | "update",
): ReadonlySet<string> {
  const tokens = requestTokens(request).filter(
    (token) => token.to <= request.position,
  );
  const result = new Set<string>();
  let keywordIndex = -1;
  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    if (tokenText(tokens[index]) === keyword) {
      keywordIndex = index;
      break;
    }
  }
  const start = tokens.findIndex(
    (token, index) => token.text === "(" && index > keywordIndex,
  );
  if (start < 0) return result;
  for (let index = start + 1; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.text === ")") break;
    if (isWordToken(token)) result.add(token.normalized.toLocaleLowerCase());
  }
  return result;
}

function updateColumnNames(request: CompletionRequest): ReadonlySet<string> {
  const tokens = requestTokens(request).filter(
    (token) => token.to <= request.position,
  );
  const result = new Set<string>();
  let setIndex = -1;
  for (let index = tokens.length - 1; index >= 0; index -= 1) {
    if (tokenText(tokens[index]) === "set") {
      setIndex = index;
      break;
    }
  }
  if (setIndex < 0) return result;
  for (let index = setIndex + 1; index < tokens.length - 1; index += 1) {
    if (isWordToken(tokens[index]) && tokens[index + 1]?.text === "=")
      result.add(tokens[index].normalized.toLocaleLowerCase());
  }
  return result;
}

export function provideRelationCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const prefix = identifierKey(context.prefix);
  const schema = context.qualifier
    ? request.catalog.schemasByName.get(identifierKey(context.qualifier))
    : undefined;
  const sqlServerSchemaFirst =
    request.document.dialect === "sqlserver" &&
    !context.qualifier &&
    request.catalog.snapshot.schemas.length > 0 &&
    [
      "from-relation",
      "join-relation",
      "insert-target",
      "update-target",
      "delete-target",
    ].includes(context.kind);
  const objects = sqlServerSchemaFirst
    ? []
    : prefix.length > 0
      ? (request.catalog.objectsByPrefix.get(prefix) ?? [])
      : [
          ...request.catalog.snapshot.tables,
          ...request.catalog.snapshot.views,
          ...request.catalog.snapshot.materializedViews,
        ];
  const result: CompletionItem[] = [];
  for (const object of objects) {
    if (schema && object.schemaId !== schema.id) continue;
    if (!matchesExecutionContext(request, object)) continue;
    const kind = "temporary" in object ? "table" : "view";
    result.push(
      item(
        context,
        request.document,
        object.name.normalized,
        renderIdentifier(object.name.normalized, request.document),
        kind,
        kind === "table" ? 100 : 95,
        kind,
        object.id,
        undefined,
        "catalog",
        {
          semanticFit: 1,
          schemaAffinity: schema ? 1 : 0.5,
          objectKindPrior: kind === "table" ? 1 : 0.8,
        },
      ),
    );
  }
  for (const cte of context.scope.ctes) {
    if (!prefix || cte.name.toLocaleLowerCase().startsWith(prefix))
      result.push(
        item(
          context,
          request.document,
          cte.name,
          cte.name,
          "cte",
          120,
          "CTE",
          undefined,
          undefined,
          "syntax",
          { semanticFit: 1 },
        ),
      );
  }
  if (!context.qualifier) {
    result.push(
      ...provideSchemaCandidates(request, {
        ...context,
        kind: "schema-name",
      }),
    );
  }
  return result;
}

export function provideSchemaCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const prefix = identifierKey(context.prefix);
  return request.catalog.snapshot.schemas
    .filter(
      (schema) => !prefix || identifierKey(schema.name).startsWith(prefix),
    )
    .map((schema) =>
      item(
        context,
        request.document,
        schema.name.normalized,
        renderIdentifier(schema.name.normalized, request.document),
        "schema",
        120,
        "schema",
        schema.id,
        undefined,
        "catalog",
        { semanticFit: 1 },
      ),
    );
}

export function provideFunctionCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const prefix = identifierKey(context.prefix);
  const builtins = [...getDialect(request.document.dialect).functions.values()]
    .filter((fn) => !prefix || identifierKey(fn.name).startsWith(prefix))
    .map((fn) =>
      item(
        context,
        request.document,
        fn.name,
        `${fn.name}(`,
        "function",
        85,
        `returns ${fn.returnFamily}`,
        undefined,
        { family: fn.returnFamily, name: fn.name },
        "dialect",
        { semanticFit: 0.9 },
      ),
    );
  const catalogFunctions = (
    prefix
      ? (request.catalog.functionsByPrefix.get(prefix) ?? [])
      : request.catalog.snapshot.functions
  ).map((fn) =>
    item(
      context,
      request.document,
      fn.name.normalized,
      `${renderIdentifier(fn.name.normalized, request.document)}(`,
      "function",
      95,
      `returns ${fn.returnType.name}`,
      fn.id,
      fn.returnType,
      "catalog",
      { semanticFit: 1 },
    ),
  );
  const seen = new Set<string>();
  return [...catalogFunctions, ...builtins].filter((candidate) => {
    const key = candidate.displayLabel.toLocaleLowerCase();
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function provideTypeCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const prefix = identifierKey(context.prefix);
  const types: CatalogType[] = prefix
    ? [...(request.catalog.typesByPrefix.get(prefix) ?? [])]
    : [...request.catalog.snapshot.types];
  return types.map((type) =>
    item(
      context,
      request.document,
      type.name.normalized,
      renderIdentifier(type.name.normalized, request.document),
      "type",
      95,
      type.family,
      type.id,
      { family: type.family, name: type.name.normalized },
      "catalog",
      { semanticFit: 1 },
    ),
  );
}

export function provideKeywordCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  return [...getDialect(request.document.dialect).keywords].map((keyword) =>
    item(
      context,
      request.document,
      keyword.toUpperCase(),
      keyword.toUpperCase(),
      "keyword",
      20,
      "keyword",
      undefined,
      undefined,
      "dialect",
      { objectKindPrior: 0.2 },
    ),
  );
}

export function provideSelectListContinuationCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const dialect = getDialect(request.document.dialect);
  return selectListContinuationWords
    .filter((word) => dialect.keywords.has(word))
    .map((word) =>
      item(
        context,
        request.document,
        word.toUpperCase(),
        word.toUpperCase(),
        "keyword",
        word === "from" ? 120 : 90,
        "clause",
        undefined,
        undefined,
        "dialect",
        { semanticFit: 1 },
      ),
    );
}

export function provideAliasCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const aliases = context.scope.outputSymbols;
  return aliases.map((symbol) =>
    item(
      context,
      request.document,
      symbol.name,
      renderIdentifier(symbol.name, request.document),
      "alias",
      100,
      "select alias",
      symbol.sourceColumn?.id,
      symbol.type,
      "syntax",
      { semanticFit: 1 },
    ),
  );
}

export function provideInsertColumnCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const object = sourceObjectFromDocument(request, "insert");
  if (!object) return [];
  const existing = insertColumnNames(request, "insert");
  return object.columns
    .filter(
      (column) =>
        !column.generated &&
        !column.identity &&
        !column.computed &&
        !existing.has(column.name.normalized.toLocaleLowerCase()),
    )
    .map((column) =>
      item(
        context,
        request.document,
        column.name.normalized,
        renderIdentifier(column.name.normalized, request.document),
        "column",
        100,
        column.type.name,
        column.id,
        column.type,
        "catalog",
        { semanticFit: 1 },
      ),
    );
}

export function provideUpdateColumnCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const object = sourceObjectFromDocument(request, "update");
  if (!object) return [];
  const existing = updateColumnNames(request);
  return object.columns
    .filter(
      (column) =>
        !column.generated &&
        !column.identity &&
        !column.computed &&
        !existing.has(column.name.normalized.toLocaleLowerCase()),
    )
    .map((column) =>
      item(
        context,
        request.document,
        column.name.normalized,
        renderIdentifier(column.name.normalized, request.document),
        "column",
        100,
        column.type.name,
        column.id,
        column.type,
        "catalog",
        { semanticFit: 1 },
      ),
    );
}

export function provideValueCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const result: CompletionItem[] = [
    item(
      context,
      request.document,
      "NULL",
      "NULL",
      "value",
      60,
      "null",
      undefined,
      { family: "null", name: "null" },
      "dialect",
    ),
    item(
      context,
      request.document,
      "DEFAULT",
      "DEFAULT",
      "value",
      55,
      "default",
      undefined,
      { family: "unknown", name: "default" },
      "dialect",
    ),
  ];
  const expected = context.expectedType?.type.family;
  if (expected === "boolean") {
    result.push(
      item(
        context,
        request.document,
        "TRUE",
        "TRUE",
        "value",
        75,
        "boolean",
        undefined,
        { family: "boolean", name: "boolean" },
        "dialect",
      ),
    );
    result.push(
      item(
        context,
        request.document,
        "FALSE",
        "FALSE",
        "value",
        75,
        "boolean",
        undefined,
        { family: "boolean", name: "boolean" },
        "dialect",
      ),
    );
  }
  for (const value of enumValues(request, context)) {
    result.push(
      item(
        context,
        request.document,
        value,
        `'${value.replace(/'/g, "''")}'`,
        "value",
        78,
        "enum value",
        undefined,
        { family: "enum", name: "enum" },
        "catalog",
      ),
    );
  }
  return result;
}

export function provideOperatorCandidates(
  request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  const prefix = context.prefix.toLocaleLowerCase();
  return getDialect(request.document.dialect)
    .operators.filter(
      (operator) => !prefix || operator.toLocaleLowerCase().startsWith(prefix),
    )
    .map((operator) =>
      item(
        context,
        request.document,
        operator,
        operator,
        "operator",
        45,
        "operator",
        undefined,
        undefined,
        "dialect",
        { semanticFit: 1 },
      ),
    );
}

export function provideJoinCandidates(
  request: CompletionRequest,
  context: CompletionContext,
  relations: RelationshipGraph,
): readonly CompletionItem[] {
  return joinCandidates(
    context.scope,
    context.graph,
    request.catalog,
    relations,
  ).map((candidate) =>
    item(
      context,
      request.document,
      candidate.clauses.join(" AND "),
      candidate.clauses.join(" AND "),
      "join",
      candidate.confidence * 150,
      candidate.relation?.name.normalized,
      candidate.relation?.id,
      undefined,
      candidate.provenance,
      { semanticFit: 1, relationshipConfidence: candidate.confidence },
    ),
  );
}
