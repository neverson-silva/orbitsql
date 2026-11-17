import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  type StateStorage,
} from "zustand/middleware";

export type ConnectionStatus =
  | "disconnected"
  | "connecting"
  | "introspecting"
  | "connected"
  | "ready"
  | "disconnecting"
  | "error";
export type ExplorerChildKind =
  | "database"
  | "schema"
  | "user"
  | "group"
  | "table"
  | "view"
  | "procedure"
  | "foreignTable"
  | "materializedView"
  | "index"
  | "function"
  | "sequence"
  | "dataType"
  | "metadata";

export type ExplorerGroupKind =
  | "tables"
  | "views"
  | "procedures"
  | "foreignTables"
  | "materializedViews"
  | "indexes"
  | "functions"
  | "sequences"
  | "dataTypes"
  | "aggregateFunctions";

export type ExplorerChild = {
  id: string;
  kind: ExplorerChildKind;
  group?: ExplorerGroupKind;
  name: string;
  parentId: string | null;
  expanded: boolean;
  loaded: boolean;
  loading?: boolean;
  databaseId?: string;
  remoteId?: string;
};

export type ExplorerConnection = {
  id: string;
  name: string;
  engine: string;
  detail: string;
  profileId?: string;
  profileColor?: string;
  host: string;
  status: ConnectionStatus;
  expanded: boolean;
  children: ExplorerChild[];
  lastRefreshedAt: number | null;
};

export type ExplorerFolder = {
  id: string;
  name: string;
  color: string;
  expanded: boolean;
  parentFolderId: string | null;
  folderIds: string[];
  connectionIds: string[];
};

export type ExplorerRootItem =
  | { kind: "connection"; id: string }
  | { kind: "folder"; id: string };

export function isConnectionReady(status: ConnectionStatus): boolean {
  return status === "connected" || status === "ready";
}

export type ExplorerDragItem = {
  kind: "connection" | "folder";
  id: string;
  parentFolderId: string | null;
};

export type ExplorerDropTarget = {
  kind: "connection" | "folder";
  id: string;
  parentFolderId: string | null;
  position?: "before" | "after";
  mode?: "insert" | "inside";
};

type ConnectionsState = {
  activeConnectionId: string | null;
  rootOrder: ExplorerRootItem[];
  connections: Record<string, ExplorerConnection>;
  folders: Record<string, ExplorerFolder>;
  addConnection: (
    connection: Omit<ExplorerConnection, "lastRefreshedAt"> & {
      lastRefreshedAt?: number | null;
    },
    folderId?: string | null,
  ) => void;
  upsertConnection: (
    connection: Omit<ExplorerConnection, "lastRefreshedAt"> & {
      lastRefreshedAt?: number | null;
    },
  ) => void;
  updateConnection: (
    connectionId: string,
    changes: Partial<
      Pick<
        ExplorerConnection,
        "name" | "host" | "detail" | "profileId" | "profileColor"
      >
    >,
  ) => void;
  deleteConnection: (connectionId: string) => void;
  toggleConnection: (connectionId: string) => void;
  setConnectionStatus: (connectionId: string, status: ConnectionStatus) => void;
  resetConnectionTree: (connectionId: string) => void;
  setConnectionRefreshed: (connectionId: string) => void;
  setConnectionChildren: (
    connectionId: string,
    children: ExplorerChild[],
  ) => void;
  mergeConnectionChildren: (
    connectionId: string,
    parentId: string,
    children: ExplorerChild[],
  ) => void;
  toggleConnectionChild: (connectionId: string, childId: string) => void;
  updateConnectionChild: (
    connectionId: string,
    childId: string,
    changes: Partial<Pick<ExplorerChild, "expanded" | "loaded" | "loading">>,
  ) => void;
  setActiveConnection: (connectionId: string | null) => void;
  addFolder: (name: string, color: string) => string | null;
  updateFolder: (
    folderId: string,
    changes: Partial<Pick<ExplorerFolder, "name" | "color">>,
  ) => void;
  deleteFolder: (folderId: string) => void;
  toggleFolder: (folderId: string) => void;
  moveItem: (source: ExplorerDragItem, target: ExplorerDropTarget) => void;
};

const memoryStorage = new Map<string, string>();
const storage: StateStorage = {
  getItem: (name) => {
    if (typeof window === "undefined") return memoryStorage.get(name) ?? null;
    return window.localStorage.getItem(name);
  },
  setItem: (name, value) => {
    if (typeof window === "undefined") {
      memoryStorage.set(name, value);
      return;
    }
    window.localStorage.setItem(name, value);
  },
  removeItem: (name) => {
    if (typeof window === "undefined") {
      memoryStorage.delete(name);
      return;
    }
    window.localStorage.removeItem(name);
  },
};

function parentFolderIdForConnection(
  state: Pick<ConnectionsState, "folders">,
  connectionId: string,
): string | null {
  return (
    Object.values(state.folders).find((folder) =>
      folder.connectionIds.includes(connectionId),
    )?.id ?? null
  );
}

function folderContains(
  folders: Record<string, ExplorerFolder>,
  parentId: string,
  childId: string,
): boolean {
  const pending = [...(folders[parentId]?.folderIds ?? [])];
  while (pending.length > 0) {
    const folderId = pending.pop();
    if (!folderId) continue;
    if (folderId === childId) return true;
    pending.push(...(folders[folderId]?.folderIds ?? []));
  }
  return false;
}

function removeFolderFromParent(
  rootOrder: ExplorerRootItem[],
  folders: Record<string, ExplorerFolder>,
  folderId: string,
): { rootOrder: ExplorerRootItem[]; folders: Record<string, ExplorerFolder> } {
  const folder = folders[folderId];
  if (!folder) return { rootOrder, folders };
  if (!folder.parentFolderId) {
    return {
      rootOrder: rootOrder.filter(
        (item) => !(item.kind === "folder" && item.id === folderId),
      ),
      folders,
    };
  }
  const parent = folders[folder.parentFolderId];
  if (!parent) return { rootOrder, folders };
  return {
    rootOrder,
    folders: {
      ...folders,
      [parent.id]: {
        ...parent,
        folderIds: parent.folderIds.filter((id) => id !== folderId),
      },
    },
  };
}

function removeConnectionFromFolders(
  folders: Record<string, ExplorerFolder>,
  connectionId: string,
): Record<string, ExplorerFolder> {
  return Object.fromEntries(
    Object.entries(folders).map(([folderId, folder]) => [
      folderId,
      {
        ...folder,
        connectionIds: folder.connectionIds.filter((id) => id !== connectionId),
      },
    ]),
  );
}

function insertBefore<T>(items: T[], targetIndex: number, item: T): T[] {
  const next = [...items];
  next.splice(Math.max(0, targetIndex), 0, item);
  return next;
}

function moveRootItem(
  rootOrder: ExplorerRootItem[],
  source: ExplorerRootItem,
  target: ExplorerRootItem,
  position: "before" | "after" = "before",
): ExplorerRootItem[] {
  const sourceIndex = rootOrder.findIndex(
    (item) => item.kind === source.kind && item.id === source.id,
  );
  const targetIndexBeforeRemoval = rootOrder.findIndex(
    (item) => item.kind === target.kind && item.id === target.id,
  );
  if (sourceIndex !== -1 && targetIndexBeforeRemoval !== -1) {
    const swapped = [...rootOrder];
    swapped[sourceIndex] = rootOrder[targetIndexBeforeRemoval];
    swapped[targetIndexBeforeRemoval] = rootOrder[sourceIndex];
    return swapped;
  }
  const withoutSource = rootOrder.filter(
    (item) => !(item.kind === source.kind && item.id === source.id),
  );
  const targetIndex = withoutSource.findIndex(
    (item) => item.kind === target.kind && item.id === target.id,
  );
  const insertionIndex =
    targetIndex === -1
      ? withoutSource.length
      : targetIndex + (position === "after" ? 1 : 0);
  return insertBefore(withoutSource, insertionIndex, source);
}

const initialState = {
  activeConnectionId: null as string | null,
  rootOrder: [] as ExplorerRootItem[],
  connections: {} as Record<string, ExplorerConnection>,
  folders: {} as Record<string, ExplorerFolder>,
};

const removedLegacyConnectionIds = new Set([
  "postgres-production",
  "mssql-development",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function migrateExplorerChild(value: unknown): ExplorerChild | null {
  if (!isRecord(value)) return null;
  if (
    typeof value.id !== "string" ||
    typeof value.name !== "string" ||
    (value.kind !== "database" &&
      value.kind !== "schema" &&
      value.kind !== "table" &&
      value.kind !== "metadata")
  ) {
    return null;
  }
  return {
    id: value.id,
    kind: value.kind,
    name: value.name,
    parentId: typeof value.parentId === "string" ? value.parentId : null,
    expanded: value.expanded === true,
    loaded: value.loaded === true,
    loading: false,
    databaseId:
      typeof value.databaseId === "string" ? value.databaseId : undefined,
    remoteId: typeof value.remoteId === "string" ? value.remoteId : undefined,
  };
}

function migrateConnectionsState(value: unknown) {
  if (!isRecord(value)) return initialState;

  const persistedConnections = isRecord(value.connections)
    ? value.connections
    : {};
  const connections = Object.fromEntries(
    Object.entries(persistedConnections)
      .filter(([connectionId]) => !removedLegacyConnectionIds.has(connectionId))
      .map(([connectionId, connection]) => {
        if (!isRecord(connection)) return [connectionId, connection];
        const children = Array.isArray(connection.children)
          ? connection.children
              .map(migrateExplorerChild)
              .filter((child): child is ExplorerChild => child !== null)
          : [];
        return [
          connectionId,
          {
            ...connection,
            profileId:
              typeof connection.profileId === "string"
                ? connection.profileId
                : undefined,
            profileColor:
              typeof connection.profileColor === "string"
                ? connection.profileColor
                : undefined,
            children,
            status: "disconnected",
          },
        ];
      }),
  ) as Record<string, ExplorerConnection>;
  const rootOrder = Array.isArray(value.rootOrder)
    ? value.rootOrder.filter(
        (item): item is ExplorerRootItem =>
          isRecord(item) &&
          (item.kind === "folder" ||
            (item.kind === "connection" &&
              typeof item.id === "string" &&
              !removedLegacyConnectionIds.has(item.id))),
      )
    : [];
  const persistedFolders = isRecord(value.folders) ? value.folders : {};
  const folders = Object.fromEntries(
    Object.entries(persistedFolders).map(([folderId, value]) => {
      if (!isRecord(value)) return [folderId, value];
      return [
        folderId,
        {
          ...value,
          parentFolderId:
            typeof value.parentFolderId === "string"
              ? value.parentFolderId
              : null,
          folderIds: Array.isArray(value.folderIds)
            ? value.folderIds.filter(
                (id): id is string => typeof id === "string",
              )
            : [],
          connectionIds: Array.isArray(value.connectionIds)
            ? value.connectionIds.filter(
                (connectionId): connectionId is string =>
                  typeof connectionId === "string" &&
                  !removedLegacyConnectionIds.has(connectionId),
              )
            : [],
        },
      ];
    }),
  ) as Record<string, ExplorerFolder>;

  return { rootOrder, connections, folders };
}

export const useConnectionsStore = create<ConnectionsState>()(
  persist(
    (set) => ({
      ...initialState,
      setActiveConnection: (connectionId) =>
        set({ activeConnectionId: connectionId }),
      addConnection: (connection, folderId = null) =>
        set((state) => {
          const nextConnection = {
            ...connection,
            lastRefreshedAt: connection.lastRefreshedAt ?? null,
          };
          const connections = {
            ...state.connections,
            [connection.id]: nextConnection,
          };
          if (folderId && state.folders[folderId]) {
            return {
              connections,
              folders: {
                ...state.folders,
                [folderId]: {
                  ...state.folders[folderId],
                  connectionIds: [
                    ...state.folders[folderId].connectionIds,
                    connection.id,
                  ],
                },
              },
            };
          }
          return {
            connections,
            rootOrder: [
              ...state.rootOrder,
              { kind: "connection" as const, id: connection.id },
            ],
          };
        }),
      upsertConnection: (connection) =>
        set((state) => {
          const existing = state.connections[connection.id];
          const nextConnection = {
            ...existing,
            ...connection,
            children:
              connection.children.length > 0
                ? connection.children
                : (existing?.children ?? []),
            lastRefreshedAt:
              connection.lastRefreshedAt ?? existing?.lastRefreshedAt ?? null,
          };
          const exists = Boolean(state.connections[connection.id]);
          return {
            connections: {
              ...state.connections,
              [connection.id]: nextConnection,
            },
            rootOrder: exists
              ? state.rootOrder
              : [
                  ...state.rootOrder,
                  { kind: "connection" as const, id: connection.id },
                ],
          };
        }),
      updateConnection: (connectionId, changes) =>
        set((state) => {
          const connection = state.connections[connectionId];
          if (!connection) return state;
          return {
            connections: {
              ...state.connections,
              [connectionId]: { ...connection, ...changes },
            },
          };
        }),
      deleteConnection: (connectionId) =>
        set((state) => {
          const connections = { ...state.connections };
          delete connections[connectionId];
          return {
            connections,
            folders: removeConnectionFromFolders(state.folders, connectionId),
            rootOrder: state.rootOrder.filter(
              (item) =>
                !(item.kind === "connection" && item.id === connectionId),
            ),
          };
        }),
      toggleConnection: (connectionId) =>
        set((state) => {
          const connection = state.connections[connectionId];
          if (!connection) return state;
          const expanded = !connection.expanded;
          return {
            connections: {
              ...state.connections,
              [connectionId]: {
                ...connection,
                expanded,
                children: expanded
                  ? connection.children
                  : connection.children.map((child) => ({
                      ...child,
                      expanded: false,
                    })),
              },
            },
          };
        }),
      setConnectionStatus: (connectionId, status) =>
        set((state) => {
          const connection = state.connections[connectionId];
          if (!connection) return state;
          return {
            connections: {
              ...state.connections,
              [connectionId]: { ...connection, status },
            },
          };
        }),
      resetConnectionTree: (connectionId) =>
        set((state) => {
          const connection = state.connections[connectionId];
          if (!connection) return state;
          return {
            connections: {
              ...state.connections,
              [connectionId]: {
                ...connection,
                expanded: false,
                children: [],
              },
            },
          };
        }),
      setConnectionRefreshed: (connectionId) =>
        set((state) => {
          const connection = state.connections[connectionId];
          if (!connection) return state;
          return {
            connections: {
              ...state.connections,
              [connectionId]: {
                ...connection,
                lastRefreshedAt: Date.now(),
              },
            },
          };
        }),
      setConnectionChildren: (connectionId, children) =>
        set((state) => {
          const connection = state.connections[connectionId];
          if (!connection) return state;
          return {
            connections: {
              ...state.connections,
              [connectionId]: {
                ...connection,
                expanded: children.length > 0 ? true : connection.expanded,
                children,
              },
            },
          };
        }),
      mergeConnectionChildren: (connectionId, parentId, children) =>
        set((state) => {
          const connection = state.connections[connectionId];
          if (!connection) return state;
          const preserved = connection.children.filter(
            (child) => child.parentId !== parentId,
          );
          return {
            connections: {
              ...state.connections,
              [connectionId]: {
                ...connection,
                children: [...preserved, ...children],
              },
            },
          };
        }),
      toggleConnectionChild: (connectionId, childId) =>
        set((state) => {
          const connection = state.connections[connectionId];
          const child = connection?.children.find(
            (item) => item.id === childId,
          );
          if (!connection || !child) return state;
          return {
            connections: {
              ...state.connections,
              [connectionId]: {
                ...connection,
                children: connection.children.map((item) =>
                  item.id === childId
                    ? { ...item, expanded: !item.expanded }
                    : item,
                ),
              },
            },
          };
        }),
      updateConnectionChild: (connectionId, childId, changes) =>
        set((state) => {
          const connection = state.connections[connectionId];
          if (!connection) return state;
          return {
            connections: {
              ...state.connections,
              [connectionId]: {
                ...connection,
                children: connection.children.map((item) =>
                  item.id === childId ? { ...item, ...changes } : item,
                ),
              },
            },
          };
        }),
      addFolder: (name, color) => {
        const normalizedName = name.trim();
        if (!normalizedName) return null;
        const folderId = `folder-${Date.now()}`;
        set((state) => ({
          folders: {
            ...state.folders,
            [folderId]: {
              id: folderId,
              name: normalizedName,
              color,
              expanded: true,
              parentFolderId: null,
              folderIds: [],
              connectionIds: [],
            },
          },
          rootOrder: [...state.rootOrder, { kind: "folder", id: folderId }],
        }));
        return folderId;
      },
      updateFolder: (folderId, changes) =>
        set((state) => {
          const folder = state.folders[folderId];
          if (!folder) return state;
          return {
            folders: {
              ...state.folders,
              [folderId]: { ...folder, ...changes },
            },
          };
        }),
      deleteFolder: (folderId) =>
        set((state) => {
          const folder = state.folders[folderId];
          if (!folder) return state;
          const folders = { ...state.folders };
          delete folders[folderId];
          const rootOrder = [...state.rootOrder];
          if (folder.parentFolderId) {
            const parent = folders[folder.parentFolderId];
            if (parent) {
              folders[parent.id] = {
                ...parent,
                folderIds: [
                  ...parent.folderIds.filter((id) => id !== folderId),
                  ...folder.folderIds,
                ],
                connectionIds: [
                  ...parent.connectionIds,
                  ...folder.connectionIds,
                ],
              };
            }
            for (const childFolderId of folder.folderIds) {
              const childFolder = folders[childFolderId];
              if (childFolder) {
                folders[childFolderId] = {
                  ...childFolder,
                  parentFolderId: folder.parentFolderId,
                };
              }
            }
          } else {
            const rootIndex = rootOrder.findIndex(
              (item) => item.kind === "folder" && item.id === folderId,
            );
            const restoredItems = [
              ...folder.folderIds.map((id) => ({
                kind: "folder" as const,
                id,
              })),
              ...folder.connectionIds.map((id) => ({
                kind: "connection" as const,
                id,
              })),
            ];
            rootOrder.splice(
              rootIndex === -1 ? rootOrder.length : rootIndex,
              1,
              ...restoredItems,
            );
            for (const childFolderId of folder.folderIds) {
              const childFolder = folders[childFolderId];
              if (childFolder) {
                folders[childFolderId] = {
                  ...childFolder,
                  parentFolderId: null,
                };
              }
            }
          }
          return { folders, rootOrder };
        }),
      toggleFolder: (folderId) =>
        set((state) => {
          const folder = state.folders[folderId];
          if (!folder) return state;
          return {
            folders: {
              ...state.folders,
              [folderId]: { ...folder, expanded: !folder.expanded },
            },
          };
        }),
      moveItem: (source, target) =>
        set((state) => {
          console.log("[sidebar:dnd] store:moveItem", {
            source,
            target,
            rootOrder: state.rootOrder,
            folders: state.folders,
          });
          if (source.kind === target.kind && source.id === target.id) {
            return state;
          }

          if (source.kind === "folder") {
            const sourceFolder = state.folders[source.id];
            if (!sourceFolder) return state;
            if (target.kind === "folder" && target.mode !== "insert") {
              if (
                target.id === source.id ||
                folderContains(state.folders, source.id, target.id)
              ) {
                return state;
              }
              const removed = removeFolderFromParent(
                state.rootOrder,
                state.folders,
                source.id,
              );
              const targetFolder = removed.folders[target.id];
              if (!targetFolder) return state;
              return {
                folders: {
                  ...removed.folders,
                  [source.id]: {
                    ...sourceFolder,
                    parentFolderId: target.id,
                  },
                  [target.id]: {
                    ...targetFolder,
                    expanded: true,
                    folderIds: [...targetFolder.folderIds, source.id],
                  },
                },
                rootOrder: removed.rootOrder,
              };
            }

            const destinationParentId = target.parentFolderId;
            const removed = removeFolderFromParent(
              state.rootOrder,
              state.folders,
              source.id,
            );
            if (destinationParentId) {
              const parent = removed.folders[destinationParentId];
              if (!parent) return state;
              const siblingIndex =
                target.kind === "folder"
                  ? parent.folderIds.indexOf(target.id)
                  : -1;
              const insertionIndex =
                siblingIndex === -1
                  ? parent.folderIds.length
                  : siblingIndex + (target.position === "after" ? 1 : 0);
              const folderIds = [...parent.folderIds];
              folderIds.splice(insertionIndex, 0, source.id);
              return {
                folders: {
                  ...removed.folders,
                  [source.id]: {
                    ...sourceFolder,
                    parentFolderId: destinationParentId,
                  },
                  [destinationParentId]: {
                    ...parent,
                    folderIds,
                  },
                },
                rootOrder: removed.rootOrder,
              };
            }
            const targetItem = { kind: target.kind, id: target.id } as const;
            return {
              folders: {
                ...removed.folders,
                [source.id]: { ...sourceFolder, parentFolderId: null },
              },
              rootOrder: moveRootItem(
                removed.rootOrder,
                { kind: "folder", id: source.id },
                targetItem,
                target.position,
              ),
            };
          }

          const sourceFolderId = parentFolderIdForConnection(state, source.id);
          if (
            sourceFolderId &&
            target.kind === "connection" &&
            target.parentFolderId === sourceFolderId
          ) {
            const folder = state.folders[sourceFolderId];
            const sourceIndex = folder?.connectionIds.indexOf(source.id) ?? -1;
            const targetIndex = folder?.connectionIds.indexOf(target.id) ?? -1;
            if (folder && sourceIndex !== -1 && targetIndex !== -1) {
              const connectionIds = [...folder.connectionIds];
              connectionIds[sourceIndex] = folder.connectionIds[targetIndex];
              connectionIds[targetIndex] = folder.connectionIds[sourceIndex];
              return {
                folders: {
                  ...state.folders,
                  [sourceFolderId]: { ...folder, connectionIds },
                },
                rootOrder: state.rootOrder,
              };
            }
          }
          let folders = removeConnectionFromFolders(state.folders, source.id);
          let rootOrder = state.rootOrder;
          if (!sourceFolderId) {
            rootOrder = rootOrder.filter(
              (item) => !(item.kind === "connection" && item.id === source.id),
            );
          }

          if (target.kind === "folder" && target.mode !== "insert") {
            const targetFolder = folders[target.id];
            if (!targetFolder) return state;
            return {
              folders: {
                ...folders,
                [target.id]: {
                  ...targetFolder,
                  connectionIds: [...targetFolder.connectionIds, source.id],
                },
              },
              rootOrder,
            };
          }

          if (target.kind === "folder" && target.mode === "insert") {
            if (target.parentFolderId) {
              const parent = folders[target.parentFolderId];
              if (!parent) return state;
              return {
                folders: {
                  ...folders,
                  [target.parentFolderId]: {
                    ...parent,
                    connectionIds: [...parent.connectionIds, source.id],
                  },
                },
                rootOrder,
              };
            }
            const targetIndex = rootOrder.findIndex(
              (item) => item.kind === "folder" && item.id === target.id,
            );
            return {
              folders,
              rootOrder: insertBefore(
                rootOrder,
                targetIndex === -1
                  ? rootOrder.length
                  : targetIndex + (target.position === "after" ? 1 : 0),
                { kind: "connection", id: source.id },
              ),
            };
          }

          if (target.parentFolderId) {
            const targetFolder = folders[target.parentFolderId];
            if (!targetFolder) return state;
            const targetIndex = targetFolder.connectionIds.indexOf(target.id);
            const insertionIndex =
              targetIndex === -1
                ? targetFolder.connectionIds.length
                : targetIndex + (target.position === "after" ? 1 : 0);
            folders = {
              ...folders,
              [target.parentFolderId]: {
                ...targetFolder,
                connectionIds: insertBefore(
                  targetFolder.connectionIds,
                  insertionIndex,
                  source.id,
                ),
              },
            };
            return { folders, rootOrder };
          }

          const targetIndex = rootOrder.findIndex(
            (item) => item.kind === "connection" && item.id === target.id,
          );
          const insertionIndex =
            targetIndex === -1
              ? rootOrder.length
              : targetIndex + (target.position === "after" ? 1 : 0);
          return {
            folders,
            rootOrder: insertBefore(rootOrder, insertionIndex, {
              kind: "connection",
              id: source.id,
            }),
          };
        }),
    }),
    {
      name: "orbitsql.connections",
      storage: createJSONStorage(() => storage),
      partialize: (state) => ({
        rootOrder: state.rootOrder,
        connections: state.connections,
        folders: state.folders,
      }),
      version: 4,
      migrate: (persistedState) => migrateConnectionsState(persistedState),
    },
  ),
);
