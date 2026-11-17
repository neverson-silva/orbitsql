import type { CatalogIndex, ForeignKeyMetadata } from "../catalog";
import type { Binding, Scope, ScopeGraph } from "./types";

export type RelationshipProvenance =
  | "real-foreign-key"
  | "unique-key"
  | "name-heuristic"
  | "type-heuristic";

export type RelationshipGraph = Readonly<{
  byTableId: ReadonlyMap<string, readonly ForeignKeyMetadata[]>;
}>;

export type JoinCandidate = Readonly<{
  source: Binding;
  target: Binding;
  relation?: ForeignKeyMetadata;
  clauses: readonly string[];
  provenance: RelationshipProvenance;
  confidence: number;
}>;

export function buildRelationshipGraph(index: CatalogIndex): RelationshipGraph {
  const byTableId = new Map<string, ForeignKeyMetadata[]>();
  for (const relation of index.snapshot.foreignKeys) {
    for (const tableId of [relation.sourceTableId, relation.targetTableId]) {
      const relations = byTableId.get(tableId) ?? [];
      relations.push(relation);
      byTableId.set(tableId, relations);
    }
  }
  return { byTableId };
}

function visibleBindings(scope: Scope, graph: ScopeGraph): readonly Binding[] {
  const result: Binding[] = [];
  let current: Scope | undefined = scope;
  while (current) {
    result.push(...current.bindings);
    current = current.parentId
      ? graph.scopes.find((candidate) => candidate.id === current?.parentId)
      : undefined;
  }
  return result;
}

function columnsForRelation(
  relation: ForeignKeyMetadata,
  source: Binding,
  target: Binding,
): readonly string[] {
  return relation.sourceColumnIds
    .map((sourceId, position) => {
      const sourceColumn = source.columns.find(
        (column) => column.sourceColumn?.id === sourceId,
      );
      const targetColumn = target.columns.find(
        (column) =>
          column.sourceColumn?.id === relation.targetColumnIds[position],
      );
      return sourceColumn && targetColumn
        ? `${source.name}.${sourceColumn.name} = ${target.name}.${targetColumn.name}`
        : undefined;
    })
    .filter((value): value is string => value !== undefined);
}

function realForeignKeyCandidates(
  bindings: readonly Binding[],
  index: CatalogIndex,
): readonly JoinCandidate[] {
  const result: JoinCandidate[] = [];
  for (const relation of index.snapshot.foreignKeys) {
    const source = bindings.find(
      (binding) => binding.object?.id === relation.sourceTableId,
    );
    const target = bindings.find(
      (binding) => binding.object?.id === relation.targetTableId,
    );
    if (!source || !target) continue;
    const clauses = columnsForRelation(relation, source, target);
    if (clauses.length === 0) continue;
    result.push({
      source,
      target,
      relation,
      clauses,
      provenance: "real-foreign-key",
      confidence: 1,
    });
  }
  return result;
}

function heuristicCandidates(
  bindings: readonly Binding[],
): readonly JoinCandidate[] {
  const result: JoinCandidate[] = [];
  for (let leftIndex = 0; leftIndex < bindings.length; leftIndex += 1) {
    const left = bindings[leftIndex];
    for (
      let rightIndex = leftIndex + 1;
      rightIndex < bindings.length;
      rightIndex += 1
    ) {
      const right = bindings[rightIndex];
      const rightUniqueKeys = new Set(
        right.columns
          .filter(
            (column) =>
              column.sourceColumn?.primaryKey ||
              (column.sourceColumn?.uniqueMemberships.length ?? 0) > 0,
          )
          .map((column) => column.name.toLocaleLowerCase()),
      );
      for (const leftColumn of left.columns) {
        const leftName = leftColumn.name.toLocaleLowerCase();
        const targetName = leftName.endsWith("_id") ? "id" : leftName;
        const targetObjectName = right.object?.name.normalized
          .toLocaleLowerCase()
          .replace(/s$/u, "");
        const relationNameMatch =
          leftName.endsWith("_id") &&
          targetObjectName !== undefined &&
          leftName === `${targetObjectName}_id`;
        const rightColumn =
          right.columns.find(
            (column) =>
              rightUniqueKeys.has(column.name.toLocaleLowerCase()) &&
              ((relationNameMatch &&
                column.name.toLocaleLowerCase() === "id") ||
                (column.name.toLocaleLowerCase() === targetName &&
                  column.type.family === leftColumn.type.family)),
          ) ??
          right.columns.find(
            (column) =>
              column.name.toLocaleLowerCase() === targetName ||
              column.type.family === leftColumn.type.family,
          );
        if (!rightColumn) continue;
        const nameMatch =
          relationNameMatch && rightColumn.name.toLocaleLowerCase() === "id";
        const uniqueMatch = rightUniqueKeys.has(
          rightColumn.name.toLocaleLowerCase(),
        );
        result.push({
          source: left,
          target: right,
          clauses: [
            `${left.name}.${leftColumn.name} = ${right.name}.${rightColumn.name}`,
          ],
          provenance: uniqueMatch
            ? "unique-key"
            : nameMatch
              ? "name-heuristic"
              : "type-heuristic",
          confidence: uniqueMatch ? 0.65 : nameMatch ? 0.35 : 0.2,
        });
      }
    }
  }
  return result;
}

export function joinCandidates(
  scope: Scope,
  scopeGraph: ScopeGraph,
  index: CatalogIndex,
  graph: RelationshipGraph,
): readonly JoinCandidate[] {
  const bindings = visibleBindings(scope, scopeGraph).filter(
    (binding) => binding.object !== undefined,
  );
  const real = realForeignKeyCandidates(bindings, index);
  const keys = new Set(
    real.map(
      (candidate) =>
        `${candidate.source.object?.id}:${candidate.target.object?.id}:${candidate.clauses.join("|")}`,
    ),
  );
  const fallback = heuristicCandidates(bindings).filter((candidate) => {
    const key = `${candidate.source.object?.id}:${candidate.target.object?.id}:${candidate.clauses.join("|")}`;
    return !keys.has(key);
  });
  return [...real, ...fallback].filter((candidate) => {
    const sourceRelations = candidate.source.object
      ? graph.byTableId.get(candidate.source.object.id)
      : undefined;
    return (
      candidate.provenance === "real-foreign-key" ||
      sourceRelations === undefined ||
      sourceRelations.length === 0
    );
  });
}
