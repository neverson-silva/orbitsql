import type {
  CatalogObject,
  CatalogSchema,
  CatalogTable,
  ColumnMetadata,
  SqlTypeRef,
} from "../catalog/types";
import type { TextRange } from "../document/types";

export type BindingKind = "table" | "view" | "cte" | "derived";

export type ProjectionSymbol = Readonly<{
  name: string;
  type: SqlTypeRef;
  sourceColumn?: ColumnMetadata;
}>;

export type Binding = Readonly<{
  name: string;
  normalizedName: string;
  kind: BindingKind;
  range: TextRange;
  object?: CatalogObject;
  schema?: CatalogSchema;
  columns: readonly ProjectionSymbol[];
  sourceName?: string;
}>;

export type Scope = Readonly<{
  id: string;
  parentId?: string;
  range: TextRange;
  bindings: readonly Binding[];
  ctes: readonly Binding[];
  outputSymbols: readonly ProjectionSymbol[];
}>;

export type ScopeGraph = Readonly<{
  scopes: readonly Scope[];
  rootScopeId: string;
  currentScopeId: string;
}>;

function projectionFromColumn(column: ColumnMetadata): ProjectionSymbol {
  return {
    name: column.name.normalized,
    type: column.type,
    sourceColumn: column,
  };
}

export function bindingColumns(binding: Binding): readonly ProjectionSymbol[] {
  return binding.columns;
}

export function bindingColumn(
  binding: Binding,
  name: string,
): ProjectionSymbol | undefined {
  const normalized = name.toLocaleLowerCase();
  return binding.columns.find(
    (column) => column.name.toLocaleLowerCase() === normalized,
  );
}

export function columnsFromObject(
  object: CatalogObject | undefined,
): readonly ProjectionSymbol[] {
  return object?.columns.map(projectionFromColumn) ?? [];
}

export function isCatalogTable(
  value: CatalogObject | undefined,
): value is CatalogTable {
  return value?.id !== undefined && "temporary" in value;
}
