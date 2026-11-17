import { create } from "zustand";
import { refreshConnectionCatalog } from "../features/connections/connection-ipc";
import {
  buildCatalogIndex,
  type CatalogIndex,
  type CatalogSnapshot,
  createEmptyCatalogIndex,
} from "../features/sql-intelligence/catalog";

type CatalogState = {
  snapshot: CatalogSnapshot;
  catalog: CatalogIndex;
  snapshots: ReadonlyMap<string, CatalogIndex>;
  refreshGenerations: ReadonlyMap<string, number>;
  setCatalog: (catalog: CatalogIndex) => void;
  setSnapshot: (snapshot: CatalogSnapshot) => void;
  mergeSnapshot: (snapshot: CatalogSnapshot) => void;
  refreshCatalog: (connectionId: string) => Promise<CatalogSnapshot>;
};

const initialCatalog = createEmptyCatalogIndex("postgresql");
const initialSnapshots = new Map([
  [initialCatalog.snapshot.connectionId, initialCatalog],
]);
const initialRefreshGenerations = new Map<string, number>();

function beginRefresh(
  state: CatalogState,
  connectionId: string,
): { generation: number; refreshGenerations: Map<string, number> } {
  const refreshGenerations = new Map(state.refreshGenerations);
  const generation = (refreshGenerations.get(connectionId) ?? 0) + 1;
  refreshGenerations.set(connectionId, generation);
  return { generation, refreshGenerations };
}

function mergeItems<T extends { id: string }>(
  current: readonly T[],
  incoming: readonly T[],
): T[] {
  const items = new Map(current.map((item) => [item.id, item]));
  for (const item of incoming) items.set(item.id, item);
  return [...items.values()];
}

function mergeSnapshots(
  current: CatalogSnapshot,
  incoming: CatalogSnapshot,
): CatalogSnapshot {
  return {
    ...current,
    id: incoming.id,
    version: incoming.version,
    generatedAt: incoming.generatedAt,
    databases: mergeItems(current.databases, incoming.databases),
    schemas: mergeItems(current.schemas, incoming.schemas),
    tables: mergeItems(current.tables, incoming.tables),
    views: mergeItems(current.views, incoming.views),
    materializedViews: mergeItems(
      current.materializedViews,
      incoming.materializedViews,
    ),
    functions: mergeItems(current.functions, incoming.functions),
    procedures: mergeItems(current.procedures, incoming.procedures),
    sequences: mergeItems(current.sequences, incoming.sequences),
    types: mergeItems(current.types, incoming.types),
    triggers: mergeItems(current.triggers, incoming.triggers),
    extensions: mergeItems(current.extensions, incoming.extensions),
    partitions: mergeItems(current.partitions, incoming.partitions),
    principals: mergeItems(current.principals, incoming.principals),
    permissions: mergeItems(current.permissions, incoming.permissions),
    synonyms: mergeItems(current.synonyms, incoming.synonyms),
    foreignKeys: mergeItems(current.foreignKeys, incoming.foreignKeys),
    indexes: mergeItems(current.indexes, incoming.indexes),
    constraints: mergeItems(current.constraints, incoming.constraints),
  };
}

export const useCatalogStore = create<CatalogState>((set) => ({
  snapshot: initialCatalog.snapshot,
  catalog: initialCatalog,
  snapshots: initialSnapshots,
  refreshGenerations: initialRefreshGenerations,
  setCatalog: (catalog) =>
    set((state) => {
      const snapshots = new Map(state.snapshots);
      snapshots.set(catalog.snapshot.connectionId, catalog);
      return { catalog, snapshot: catalog.snapshot, snapshots };
    }),
  setSnapshot: (snapshot) =>
    set((state) => {
      const catalog = buildCatalogIndex(snapshot);
      const snapshots = new Map(state.snapshots);
      snapshots.set(snapshot.connectionId, catalog);
      return { snapshot, catalog, snapshots };
    }),
  mergeSnapshot: (incoming) =>
    set((state) => {
      const snapshot = mergeSnapshots(state.snapshot, incoming);
      const catalog = buildCatalogIndex(snapshot);
      const snapshots = new Map(state.snapshots);
      snapshots.set(snapshot.connectionId, catalog);
      return { snapshot, catalog, snapshots };
    }),
  refreshCatalog: async (connectionId) => {
    let generation = 0;
    set((state) => {
      const next = beginRefresh(state, connectionId);
      generation = next.generation;
      return { refreshGenerations: next.refreshGenerations };
    });
    const snapshot = await refreshConnectionCatalog(connectionId);
    const catalog = buildCatalogIndex(snapshot);
    set((state) => {
      if (state.refreshGenerations.get(connectionId) !== generation)
        return state;
      const snapshots = new Map(state.snapshots);
      snapshots.set(snapshot.connectionId, catalog);
      return { snapshot, catalog, snapshots };
    });
    return snapshot;
  },
}));
