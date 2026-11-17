import { useNavigate } from "@tanstack/react-router";
import {
  type DragEvent,
  type PointerEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { COMMAND_IDS, type CommandId } from "../../commands/command-model";
import {
  type CommandHandler,
  registerCommandHandlers,
} from "../../commands/command-registry";
import {
  childrenFromSnapshot,
  schemaChildrenFromSnapshot,
} from "../../features/connections/catalog-explorer";
import {
  connectConnection,
  connectionErrorMessage,
  deleteConnection as deleteRemoteConnection,
  disconnectConnection,
  executeQuery,
  expandConnectionNode,
  isConnectionReconnectFailure,
  listConnectionProfileDefinitions,
  listConnectionStatuses,
  listSavedConnections,
  refreshConnectionCatalog,
  renameConnection,
} from "../../features/connections/connection-ipc";
import { contextFromExplorerNode } from "../../features/connections/execution-context";
import { useCatalogStore } from "../../stores/catalog.store";
import {
  type ExplorerChild,
  type ExplorerConnection,
  type ExplorerDragItem,
  type ExplorerDropTarget,
  isConnectionReady,
  useConnectionsStore,
} from "../../stores/connections.store";
import { openQueryTab, useQueryTabsStore } from "../../stores/query-tabs.store";
import { useSidebarFolderStore } from "../../stores/sidebar.store";
import { toast } from "../ui/toast";
import { ConnectionsList } from "./connections-list";
import {
  ConnectionDialog,
  DeleteDialog,
  FolderDialog,
  ObjectMutationDialog,
} from "./dialogs";
import type {
  SidebarDeleteTarget,
  SidebarEditMode,
  SidebarObjectMutation,
} from "./sidebar.types";

function quoteIdentifier(value: string) {
  return `"${value.replace(/"/g, '""')}"`;
}

function sameDropTarget(
  left: ExplorerDropTarget | null,
  right: ExplorerDropTarget | null,
) {
  return Boolean(
    left &&
      right &&
      left.kind === right.kind &&
      left.id === right.id &&
      left.parentFolderId === right.parentFolderId &&
      left.position === right.position &&
      left.mode === right.mode,
  );
}

function objectSchemaName(
  children: readonly ExplorerChild[],
  child: ExplorerChild,
) {
  let parentId = child.parentId;
  while (parentId) {
    const parent = children.find((candidate) => candidate.id === parentId);
    if (!parent) return undefined;
    if (parent.kind === "schema") return parent.name;
    parentId = parent.parentId;
  }
  return undefined;
}

function objectReference(
  children: readonly ExplorerChild[],
  child: ExplorerChild,
) {
  const schema = objectSchemaName(children, child);
  return schema
    ? `${quoteIdentifier(schema)}.${quoteIdentifier(child.name)}`
    : quoteIdentifier(child.name);
}

function objectSql(
  children: readonly ExplorerChild[],
  child: ExplorerChild,
  operation: "delete" | "rename",
  newName = child.name,
) {
  const reference = objectReference(children, child);
  if (operation === "rename") {
    const keyword = child.kind === "view" ? "VIEW" : "TABLE";
    return `ALTER ${keyword} ${reference} RENAME TO ${quoteIdentifier(newName)};`;
  }
  const keyword =
    child.kind === "view"
      ? "VIEW"
      : child.kind === "materializedView"
        ? "MATERIALIZED VIEW"
        : child.kind === "procedure"
          ? "PROCEDURE"
          : child.kind === "function"
            ? "FUNCTION"
            : child.kind === "sequence"
              ? "SEQUENCE"
              : child.kind === "dataType"
                ? "TYPE"
                : child.kind === "index"
                  ? "INDEX"
                  : "TABLE";
  return `DROP ${keyword} ${reference};`;
}

function tableMetadata(
  tableId: string,
  snapshot: {
    tables: readonly {
      id: string;
      columns: readonly unknown[];
      indexes: readonly unknown[];
      foreignKeys: readonly unknown[];
      checkConstraints: readonly unknown[];
    }[];
  },
  lookupId: string,
): ExplorerChild[] {
  const table = snapshot.tables.find((candidate) => candidate.id === lookupId);
  if (!table) return [];
  return [
    {
      id: `${tableId}:columns`,
      kind: "metadata",
      name: `Columns (${table.columns.length})`,
      parentId: tableId,
      expanded: false,
      loaded: true,
    },
    {
      id: `${tableId}:indexes`,
      kind: "metadata",
      name: `Indexes (${table.indexes.length})`,
      parentId: tableId,
      expanded: false,
      loaded: true,
    },
    {
      id: `${tableId}:foreign-keys`,
      kind: "metadata",
      name: `Foreign keys (${table.foreignKeys.length})`,
      parentId: tableId,
      expanded: false,
      loaded: true,
    },
    {
      id: `${tableId}:checks`,
      kind: "metadata",
      name: `Checks (${table.checkConstraints.length})`,
      parentId: tableId,
      expanded: false,
      loaded: true,
    },
  ];
}

export function ConnectionExplorer() {
  const navigate = useNavigate();
  const rootOrder = useConnectionsStore((state) => state.rootOrder);
  const connections = useConnectionsStore((state) => state.connections);
  const folders = useConnectionsStore((state) => state.folders);
  const activeConnectionId = useConnectionsStore(
    (state) => state.activeConnectionId,
  );
  const activeQueryTab = useQueryTabsStore((state) =>
    state.tabs.find((tab) => tab.id === state.activeTabId),
  );
  const selectedSchema =
    activeQueryTab?.connectionId === activeConnectionId
      ? activeQueryTab.executionContext.dimensions.schema
      : undefined;
  const store = useConnectionsStore();
  const catalog = useCatalogStore();
  const folderStore = useSidebarFolderStore();

  const [dragging, setDragging] = useState<ExplorerDragItem | null>(null);
  const [dropTarget, setDropTarget] = useState<ExplorerDropTarget | null>(null);
  const [refreshingIds, setRefreshingIds] = useState<Set<string>>(new Set());
  const [, setNotice] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<SidebarDeleteTarget | null>(
    null,
  );
  const [objectMutation, setObjectMutation] =
    useState<SidebarObjectMutation | null>(null);
  const [objectMutationName, setObjectMutationName] = useState("");
  const [objectMutationBusy, setObjectMutationBusy] = useState(false);
  const [editingConnectionId, setEditingConnectionId] = useState<string | null>(
    null,
  );
  const [editingName, setEditingName] = useState("");
  const pointerDragRef = useRef<{
    item: ExplorerDragItem;
    startX: number;
    startY: number;
    moved: boolean;
    target: ExplorerDropTarget | null;
    targetElement: HTMLElement | null;
    lastClientY: number;
  } | null>(null);
  const suppressPointerClickRef = useRef(false);

  const dropTargetFromElement = useCallback((element: Element | null) => {
    const targetElement = element?.closest<HTMLElement>(
      "[data-sidebar-dnd-kind]",
    );
    if (!targetElement) return null;
    const kind = targetElement.dataset.sidebarDndKind;
    const id = targetElement.dataset.sidebarDndId;
    if ((kind !== "connection" && kind !== "folder") || !id) return null;
    return {
      element: targetElement,
      target: {
        kind,
        id,
        parentFolderId: targetElement.dataset.sidebarDndParentFolderId || null,
      } satisfies ExplorerDropTarget,
    };
  }, []);

  const startPointerDrag = useCallback(
    (event: PointerEvent<HTMLElement>, item: ExplorerDragItem) => {
      if (event.button !== 0 || pointerDragRef.current) return;
      const drag = {
        item,
        startX: event.clientX,
        startY: event.clientY,
        moved: false,
        target: null,
        targetElement: null,
        lastClientY: event.clientY,
      };
      pointerDragRef.current = drag;
      console.log("[sidebar:dnd] pointer:start", item);

      const finish = () => {
        const current = pointerDragRef.current;
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", finish);
        window.removeEventListener("pointercancel", finish);
        pointerDragRef.current = null;
        if (!current?.moved) return;
        suppressPointerClickRef.current = true;
        if (current.target && current.targetElement) {
          const bounds = current.targetElement.getBoundingClientRect();
          const position: ExplorerDropTarget["position"] =
            current.target.mode === "insert" &&
            current.lastClientY > bounds.top + bounds.height / 2
              ? "after"
              : "before";
          const target = { ...current.target, position };
          console.log("[sidebar:dnd] pointer:drop", {
            source: current.item,
            target,
          });
          store.moveItem(current.item, target);
        } else {
          console.log("[sidebar:dnd] pointer:drop:no-target", {
            source: current.item,
          });
        }
        setDragging(null);
        setDropTarget(null);
      };

      const move = (moveEvent: globalThis.PointerEvent) => {
        const current = pointerDragRef.current;
        if (!current) return;
        current.lastClientY = moveEvent.clientY;
        const distance = Math.hypot(
          moveEvent.clientX - current.startX,
          moveEvent.clientY - current.startY,
        );
        if (!current.moved && distance < 5) return;
        if (!current.moved) {
          current.moved = true;
          setDragging(current.item);
          console.log("[sidebar:dnd] pointer:active", current.item);
        }
        moveEvent.preventDefault();
        const resolved = dropTargetFromElement(
          document.elementFromPoint(moveEvent.clientX, moveEvent.clientY),
        );
        let nextTarget: ExplorerDropTarget | null = null;
        if (resolved) {
          const target = resolved.target;
          const bounds = resolved.element.getBoundingClientRect();
          const edgeSize = Math.min(14, bounds.height * 0.3);
          const inTopEdge = moveEvent.clientY <= bounds.top + edgeSize;
          const inBottomEdge = moveEvent.clientY >= bounds.bottom - edgeSize;
          const canNestItem = target.kind === "folder";
          const inside = canNestItem && !inTopEdge && !inBottomEdge;
          nextTarget = {
            ...target,
            mode: inside ? "inside" : "insert",
            ...(inside
              ? {}
              : {
                  position: inBottomEdge ? "after" : "before",
                }),
          };
        }
        const targetChanged = !sameDropTarget(current.target, nextTarget);
        current.target = nextTarget;
        current.targetElement = resolved?.element ?? null;
        if (targetChanged) setDropTarget(current.target);
      };

      window.addEventListener("pointermove", move, { passive: false });
      window.addEventListener("pointerup", finish);
      window.addEventListener("pointercancel", finish);
    },
    [dropTargetFromElement, store.moveItem],
  );

  const consumePointerClick = useCallback(() => {
    const suppressed = suppressPointerClickRef.current;
    suppressPointerClickRef.current = false;
    return suppressed;
  }, []);

  useEffect(() => {
    void Promise.all([
      listSavedConnections(),
      listConnectionProfileDefinitions(),
      listConnectionStatuses(),
    ])
      .then(([connections, profiles, connectedIds]) => {
        const connected = new Set(connectedIds);
        const known = new Set(connections.map((connection) => connection.id));
        for (const id of Object.keys(
          useConnectionsStore.getState().connections,
        )) {
          if (!known.has(id)) store.deleteConnection(id);
        }
        for (const connection of connections) {
          const current =
            useConnectionsStore.getState().connections[connection.id];
          const currentOperationInProgress =
            current &&
            ["connecting", "introspecting", "disconnecting"].includes(
              current.status,
            );
          const currentReady = current && isConnectionReady(current.status);
          const connectedNow =
            connected.has(connection.id) ||
            currentReady ||
            currentOperationInProgress;
          const selectedProfile =
            profiles.find((profile) => profile.id === connection.profileId) ??
            profiles.find(
              (profile) =>
                profile.name.toLowerCase() ===
                connection.profileId.toLowerCase(),
            );
          const profileId = selectedProfile?.id ?? connection.profileId;
          const profileName = selectedProfile?.name ?? connection.profileId;
          store.upsertConnection({
            id: connection.id,
            name: connection.name,
            engine:
              connection.dialect === "sqlserver"
                ? "Microsoft SQL Server"
                : "PostgreSQL",
            detail: connection.database
              ? `${profileName} • ${connection.database}`
              : profileName,
            profileId,
            profileColor: selectedProfile?.color,
            host: connection.server,
            status: connected.has(connection.id)
              ? "ready"
              : connectedNow && current
                ? current.status
                : "disconnected",
            expanded: current?.expanded ?? false,
            children: current?.children ?? [],
          });
          if (!connectedNow) {
            store.resetConnectionTree(connection.id);
          }
        }
      })
      .catch(() => undefined);
  }, [
    store.deleteConnection,
    store.resetConnectionTree,
    store.upsertConnection,
  ]);

  const showNotice = useCallback((message: string) => {
    setNotice(message);
    window.setTimeout(() => setNotice(null), 2200);
  }, []);

  const showConnectionError = useCallback(
    (error: unknown) => {
      const message = connectionErrorMessage(error);
      showNotice(message);
      toast.add({
        description: message,
        title: "Connection failed",
        type: "error",
      });
    },
    [showNotice],
  );

  const startDrag = useCallback(
    (event: DragEvent<HTMLElement>, item: ExplorerDragItem) => {
      console.log("[sidebar:dnd] dragstart", item);
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData(
        "application/x-orbitsql-explorer",
        JSON.stringify(item),
      );
      event.dataTransfer.setData("text/plain", item.id);
      setDragging(item);
      setDropTarget(null);
    },
    [],
  );

  const dragOver = useCallback(
    (event: DragEvent<HTMLElement>, target: ExplorerDropTarget) => {
      console.log("[sidebar:dnd] dragover", {
        target,
        types: Array.from(event.dataTransfer.types),
      });
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
      setDropTarget(target);
    },
    [],
  );

  const drop = useCallback(
    (event: DragEvent<HTMLElement>, target: ExplorerDropTarget) => {
      console.log("[sidebar:dnd] drop:start", {
        target,
        types: Array.from(event.dataTransfer.types),
      });
      event.preventDefault();
      const payload = event.dataTransfer.getData(
        "application/x-orbitsql-explorer",
      );
      let source = dragging;
      try {
        const parsed = JSON.parse(payload) as ExplorerDragItem;
        if (
          (parsed.kind === "connection" || parsed.kind === "folder") &&
          typeof parsed.id === "string" &&
          (parsed.parentFolderId === null ||
            typeof parsed.parentFolderId === "string")
        ) {
          source = parsed;
        }
      } catch {
        source = dragging;
      }
      console.log("[sidebar:dnd] drop:source", { source, dragging, payload });
      if (source) {
        const bounds = event.currentTarget.getBoundingClientRect();
        const position: ExplorerDropTarget["position"] =
          target.kind === "connection" &&
          event.clientY > bounds.top + bounds.height / 2
            ? "after"
            : "before";
        const nextTarget = { ...target, position };
        console.log("[sidebar:dnd] moveItem", { source, target: nextTarget });
        store.moveItem(source, nextTarget);
      }
      setDragging(null);
      setDropTarget(null);
    },
    [dragging, store.moveItem],
  );

  const clearDrag = useCallback(() => {
    console.log("[sidebar:dnd] dragend");
    setDragging(null);
    setDropTarget(null);
  }, []);

  const refreshConnection = useCallback(
    async (connectionId: string) => {
      if (refreshingIds.has(connectionId)) return;
      store.setActiveConnection(connectionId);
      store.resetConnectionTree(connectionId);
      setRefreshingIds((current) => new Set(current).add(connectionId));
      store.setConnectionStatus(connectionId, "introspecting");
      try {
        const snapshot = await refreshConnectionCatalog(connectionId);
        catalog.setSnapshot(snapshot);
        store.setConnectionChildren(
          connectionId,
          childrenFromSnapshot(snapshot),
        );
        store.setConnectionRefreshed(connectionId);
        store.setConnectionStatus(connectionId, "ready");
        showNotice("Connection explorer refreshed");
      } catch (error) {
        store.setConnectionStatus(
          connectionId,
          isConnectionReconnectFailure(error) ? "disconnected" : "error",
        );
        showConnectionError(error);
      } finally {
        setRefreshingIds((current) => {
          const next = new Set(current);
          next.delete(connectionId);
          return next;
        });
      }
    },
    [
      catalog.setSnapshot,
      refreshingIds,
      showConnectionError,
      showNotice,
      store,
    ],
  );

  const refreshObject = useCallback(
    async (connectionId: string, child: ExplorerChild) => {
      const connection = connections[connectionId];
      if (!connection || refreshingIds.has(connectionId)) return;
      store.setActiveConnection(connectionId);
      setRefreshingIds((current) => new Set(current).add(connectionId));
      store.setConnectionStatus(connectionId, "introspecting");
      try {
        const snapshot = await refreshConnectionCatalog(connectionId);
        catalog.setSnapshot(snapshot);
        store.setConnectionStatus(connectionId, "ready");
        showNotice(`${child.name} refreshed`);
      } catch (error) {
        store.setConnectionStatus(
          connectionId,
          isConnectionReconnectFailure(error) ? "disconnected" : "error",
        );
        showNotice(connectionErrorMessage(error));
      } finally {
        setRefreshingIds((current) => {
          const next = new Set(current);
          next.delete(connectionId);
          return next;
        });
      }
    },
    [catalog.setSnapshot, connections, refreshingIds, showNotice, store],
  );

  const toggleRemoteConnection = useCallback(
    async (connection: ExplorerConnection) => {
      try {
        if (isConnectionReady(connection.status)) {
          store.setConnectionStatus(connection.id, "disconnecting");
          await disconnectConnection(connection.id);
          store.resetConnectionTree(connection.id);
          store.setConnectionStatus(connection.id, "disconnected");
          if (activeConnectionId === connection.id)
            store.setActiveConnection(null);
          showNotice("Connection disconnected");
          return;
        }
        store.setActiveConnection(connection.id);
        store.resetConnectionTree(connection.id);
        store.setConnectionStatus(connection.id, "connecting");
        const session = await connectConnection(connection.id);
        store.setConnectionStatus(connection.id, "introspecting");
        let snapshot = session.catalog;
        let children = childrenFromSnapshot(snapshot);
        if (children.length === 0) {
          snapshot = await refreshConnectionCatalog(connection.id);
          children = childrenFromSnapshot(snapshot);
        }
        catalog.setSnapshot(snapshot);
        store.setConnectionChildren(connection.id, children);
        store.setConnectionRefreshed(connection.id);
        store.setConnectionStatus(connection.id, "ready");
        store.setActiveConnection(connection.id);
        showNotice("Connection connected");
      } catch (error) {
        store.setConnectionStatus(
          connection.id,
          isConnectionReconnectFailure(error) ? "disconnected" : "error",
        );
        showConnectionError(error);
      }
    },
    [
      activeConnectionId,
      catalog.setSnapshot,
      showConnectionError,
      showNotice,
      store,
    ],
  );

  const expandDatabase = useCallback(
    async (connectionId: string, child: ExplorerChild) => {
      if (child.kind !== "database") return;
      const shouldLoad = !child.expanded && !child.loaded;
      store.updateConnectionChild(connectionId, child.id, {
        expanded: !child.expanded,
        loading: shouldLoad,
      });
      if (child.expanded || child.loaded) return;
      const cachedSnapshot = catalog.snapshots.get(connectionId)?.snapshot;
      if (cachedSnapshot?.dialect === "postgresql") {
        store.mergeConnectionChildren(
          connectionId,
          child.id,
          tableMetadata(child.id, cachedSnapshot, child.remoteId ?? child.id),
        );
        store.updateConnectionChild(connectionId, child.id, {
          loaded: true,
          loading: false,
        });
        return;
      }
      store.updateConnectionChild(connectionId, child.id, { loading: true });
      try {
        const databaseId = child.databaseId ?? child.name;
        const snapshot = await expandConnectionNode(connectionId, {
          databaseId,
        });
        catalog.mergeSnapshot(snapshot);
        store.mergeConnectionChildren(
          connectionId,
          child.id,
          childrenFromSnapshot(snapshot, databaseId).map((entry) => ({
            ...entry,
            parentId: child.id,
            databaseId,
          })),
        );
        store.updateConnectionChild(connectionId, child.id, {
          loaded: true,
          loading: false,
        });
      } catch (error) {
        store.updateConnectionChild(connectionId, child.id, {
          expanded: false,
          loading: false,
        });
        showNotice(connectionErrorMessage(error));
      }
    },
    [catalog.mergeSnapshot, catalog.snapshots, showNotice, store],
  );

  const expandSchema = useCallback(
    async (connectionId: string, child: ExplorerChild) => {
      if (child.kind !== "schema") return;
      const current = connections[connectionId]?.children ?? [];
      const stale = current.some(
        (entry) => entry.parentId === child.id && entry.kind !== "group",
      );
      const open = !child.expanded || stale;
      store.updateConnectionChild(connectionId, child.id, {
        expanded: open,
        loading: open,
      });
      if (!open) return;
      const databaseId = child.databaseId;
      const cachedSnapshot = catalog.snapshots.get(connectionId)?.snapshot;
      if (cachedSnapshot?.dialect === "postgresql") {
        store.mergeConnectionChildren(
          connectionId,
          child.id,
          schemaChildrenFromSnapshot(
            cachedSnapshot,
            child.id,
            child.remoteId ?? child.id,
          ).map((entry) => ({
            ...entry,
            databaseId: databaseId ?? cachedSnapshot.databaseId,
          })),
        );
        store.updateConnectionChild(connectionId, child.id, {
          loaded: true,
          loading: false,
        });
        return;
      }
      try {
        const snapshot = await expandConnectionNode(connectionId, {
          schemaId: child.remoteId ?? child.id,
          ...(databaseId ? { databaseId } : {}),
        });
        catalog.mergeSnapshot(snapshot);
        store.mergeConnectionChildren(
          connectionId,
          child.id,
          schemaChildrenFromSnapshot(
            snapshot,
            child.id,
            child.remoteId ?? child.id,
          ).map((entry) => ({
            ...entry,
            databaseId: databaseId ?? snapshot.databaseId,
          })),
        );
        store.updateConnectionChild(connectionId, child.id, {
          loaded: true,
          loading: false,
        });
      } catch (error) {
        store.updateConnectionChild(connectionId, child.id, {
          expanded: false,
          loading: false,
        });
        showNotice(connectionErrorMessage(error));
      }
    },
    [catalog.mergeSnapshot, catalog.snapshots, connections, showNotice, store],
  );

  const expandTable = useCallback(
    async (connectionId: string, child: ExplorerChild) => {
      if (child.kind !== "table") return;
      store.updateConnectionChild(connectionId, child.id, {
        expanded: !child.expanded,
      });
      if (child.expanded || child.loaded) return;
      try {
        const snapshot = await expandConnectionNode(connectionId, {
          tableId: child.remoteId ?? child.id,
          ...(child.databaseId ? { databaseId: child.databaseId } : {}),
        });
        catalog.mergeSnapshot(snapshot);
        store.mergeConnectionChildren(
          connectionId,
          child.id,
          tableMetadata(child.id, snapshot, child.remoteId ?? child.id),
        );
        store.updateConnectionChild(connectionId, child.id, {
          loaded: true,
          loading: false,
        });
      } catch (error) {
        store.updateConnectionChild(connectionId, child.id, {
          expanded: false,
          loading: false,
        });
        showNotice(connectionErrorMessage(error));
      }
    },
    [catalog.mergeSnapshot, showNotice, store],
  );

  const openConnectionEditor = useCallback(
    (connection: ExplorerConnection, mode: SidebarEditMode) => {
      if (mode === "edit") {
        void navigate({
          to: "/connections/new/config",
          search: {
            connectionId: connection.id,
            engine: connection.engine,
          },
        });
        return;
      }
      setEditingConnectionId(connection.id);
      setEditingName(connection.name);
    },
    [navigate],
  );

  const saveConnectionEdit = useCallback(async () => {
    if (!editingConnectionId || !editingName.trim()) return;
    try {
      const updated = await renameConnection(
        editingConnectionId,
        editingName.trim(),
      );
      store.updateConnection(editingConnectionId, { name: updated.name });
      setEditingConnectionId(null);
      showNotice("Connection updated");
    } catch (error) {
      showNotice(connectionErrorMessage(error));
    }
  }, [editingConnectionId, editingName, showNotice, store]);

  const saveFolder = useCallback(() => {
    if (!folderStore.folderName.trim()) return;
    if (folderStore.editingFolderId) {
      store.updateFolder(folderStore.editingFolderId, {
        name: folderStore.folderName.trim(),
        color: folderStore.folderColor,
      });
      showNotice("Folder updated");
    } else {
      store.addFolder(folderStore.folderName.trim(), folderStore.folderColor);
      showNotice("Folder created");
    }
    folderStore.closeFolderDialog();
  }, [folderStore, showNotice, store]);

  const confirmDelete = useCallback(() => {
    if (!deleteTarget) return;
    if (deleteTarget.kind === "folder") {
      store.deleteFolder(deleteTarget.id);
      showNotice("Folder deleted; connections moved to root");
      setDeleteTarget(null);
      return;
    }
    void deleteRemoteConnection(deleteTarget.id)
      .then(() => {
        store.deleteConnection(deleteTarget.id);
        if (activeConnectionId === deleteTarget.id)
          store.setActiveConnection(null);
        showNotice("Connection deleted");
      })
      .catch((error) => showNotice(connectionErrorMessage(error)));
    setDeleteTarget(null);
  }, [activeConnectionId, deleteTarget, showNotice, store]);

  const requestObjectMutation = useCallback(
    (
      connectionId: string,
      child: ExplorerChild,
      operation: "delete" | "rename",
    ) => {
      const children = connections[connectionId]?.children ?? [];
      setObjectMutation({
        child,
        connectionId,
        operation,
        sql: objectSql(children, child, operation),
      });
      setObjectMutationName(child.name);
    },
    [connections],
  );

  const changeObjectMutationName = useCallback(
    (name: string) => {
      setObjectMutationName(name);
      setObjectMutation((current) => {
        if (current?.operation !== "rename") return current;
        const children = connections[current.connectionId]?.children ?? [];
        return {
          ...current,
          sql: objectSql(children, current.child, "rename", name),
        };
      });
    },
    [connections],
  );

  const confirmObjectMutation = useCallback(async () => {
    if (!objectMutation || objectMutationBusy) return;
    if (objectMutation.operation === "rename" && !objectMutationName.trim())
      return;
    setObjectMutationBusy(true);
    try {
      await executeQuery(objectMutation.connectionId, objectMutation.sql);
      setObjectMutation(null);
      await refreshConnection(objectMutation.connectionId);
      showNotice(
        objectMutation.operation === "rename"
          ? "Object renamed"
          : "Object deleted",
      );
    } catch (error) {
      showNotice(connectionErrorMessage(error));
    } finally {
      setObjectMutationBusy(false);
    }
  }, [
    objectMutation,
    objectMutationBusy,
    objectMutationName,
    refreshConnection,
    showNotice,
  ]);

  const openObjectView = useCallback(
    (
      connectionId: string,
      child: ExplorerChild,
      view: "properties" | "data" | "diagram",
    ) => {
      const path =
        view === "data" ? "/objects/table-data" : "/objects/table-inspector";
      void navigate({
        to: path,
        search: { connectionId, tableId: child.remoteId ?? child.id, view },
      });
    },
    [navigate],
  );

  const openObjectQuery = useCallback(
    (connectionId: string, query: string) => {
      const tabId = openQueryTab(connectionId);
      useQueryTabsStore.getState().updateTab(tabId, { query });
      void navigate({ to: "/query" });
    },
    [navigate],
  );

  const copyObjectName = useCallback(
    (child: ExplorerChild) => {
      void navigator.clipboard?.writeText(child.name);
      showNotice("Object name copied");
    },
    [showNotice],
  );

  const exportObjectData = useCallback(
    (connectionId: string, child: ExplorerChild) => {
      const children = connections[connectionId]?.children ?? [];
      openObjectQuery(
        connectionId,
        `COPY (SELECT * FROM ${objectReference(children, child)}) TO STDOUT WITH CSV HEADER;`,
      );
    },
    [connections, openObjectQuery],
  );

  const importObjectData = useCallback(
    (connectionId: string, child: ExplorerChild) => {
      const children = connections[connectionId]?.children ?? [];
      openObjectQuery(
        connectionId,
        `COPY ${objectReference(children, child)} FROM STDIN WITH CSV HEADER;`,
      );
    },
    [connections, openObjectQuery],
  );

  useEffect(() => {
    const active = activeConnectionId ? connections[activeConnectionId] : null;
    const handlers: Partial<Record<CommandId, CommandHandler>> = {};
    const busy = active
      ? ["connecting", "disconnecting", "introspecting"].includes(active.status)
      : true;
    if (active && !busy) {
      if (isConnectionReady(active.status)) {
        handlers[COMMAND_IDS.databaseDisconnect] = () =>
          void toggleRemoteConnection(active);
        if (!refreshingIds.has(active.id)) {
          handlers[COMMAND_IDS.databaseRefresh] = () =>
            void refreshConnection(active.id);
          handlers[COMMAND_IDS.databaseRefreshSchema] = () =>
            void refreshConnection(active.id);
        }
      } else {
        handlers[COMMAND_IDS.databaseConnect] = () =>
          void toggleRemoteConnection(active);
      }
    }
    return registerCommandHandlers(handlers, {
      [COMMAND_IDS.databaseConnect]: {
        enabled: Boolean(handlers[COMMAND_IDS.databaseConnect]),
      },
      [COMMAND_IDS.databaseDisconnect]: {
        enabled: Boolean(handlers[COMMAND_IDS.databaseDisconnect]),
      },
      [COMMAND_IDS.databaseRefresh]: {
        enabled: Boolean(handlers[COMMAND_IDS.databaseRefresh]),
      },
      [COMMAND_IDS.databaseRefreshSchema]: {
        enabled: Boolean(handlers[COMMAND_IDS.databaseRefreshSchema]),
      },
    });
  }, [
    activeConnectionId,
    connections,
    refreshingIds,
    refreshConnection,
    toggleRemoteConnection,
  ]);

  const actions = {
    toggleConnection: (connectionId: string) => {
      const connection = connections[connectionId];
      if (
        connection &&
        isConnectionReady(connection.status) &&
        !connection.children.some((child) => child.parentId === null)
      ) {
        void refreshConnection(connectionId);
        return;
      }
      store.toggleConnection(connectionId);
    },
    toggleFolder: store.toggleFolder,
    startDrag,
    startPointerDrag,
    dragOver,
    drop,
    clearDrag,
    consumePointerClick,
    expandDatabase,
    expandSchema,
    expandTable,
    toggleChild: store.toggleConnectionChild,
    selectConnection: store.setActiveConnection,
    toggleRemoteConnection,
    refreshConnection: (id: string) => void refreshConnection(id),
    refreshObject: (id: string, child: ExplorerChild) =>
      void refreshObject(id, child),
    openQueryScript: (connectionId: string, child?: ExplorerChild) => {
      const connection = connections[connectionId];
      const context = connection
        ? contextFromExplorerNode(
            connection,
            child,
            catalog.snapshots.get(connectionId),
          )
        : undefined;
      openQueryTab(connectionId, context);
      void navigate({ to: "/query" });
    },
    openObjectView,
    copyObjectName,
    exportObjectData,
    importObjectData,
    requestObjectMutation,
    copyConnection: (connection: ExplorerConnection) => {
      void navigator.clipboard?.writeText(
        `${connection.engine.toLowerCase().replace(/ /g, "+")}://${connection.host}`,
      );
      showNotice("Connection string copied");
    },
    openConnectionEditor,
    requestDelete: setDeleteTarget,
    openFolderEditor: folderStore.openFolderEditor,
  };

  return (
    <div
      className="flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
      data-slot="sidebar-explorer-body"
    >
      <ConnectionsList
        actions={actions}
        activeConnectionId={activeConnectionId}
        connections={connections}
        dragging={dragging}
        dropTarget={dropTarget}
        folders={folders}
        refreshingIds={refreshingIds}
        rootOrder={rootOrder}
        selectedSchema={selectedSchema}
      />
      <ConnectionDialog
        name={editingName}
        onCancel={() => setEditingConnectionId(null)}
        onChange={setEditingName}
        onOpenChange={(open) => !open && setEditingConnectionId(null)}
        onSave={() => void saveConnectionEdit()}
        open={Boolean(editingConnectionId)}
      />
      <FolderDialog
        color={folderStore.folderColor}
        editing={Boolean(folderStore.editingFolderId)}
        name={folderStore.folderName}
        onCancel={folderStore.closeFolderDialog}
        onColorChange={folderStore.setFolderColor}
        onNameChange={folderStore.setFolderName}
        onOpenChange={(open) => !open && folderStore.closeFolderDialog()}
        onSave={saveFolder}
        open={folderStore.folderDialogOpen}
      />
      <DeleteDialog
        onCancel={() => setDeleteTarget(null)}
        onConfirm={confirmDelete}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        open={Boolean(deleteTarget)}
        target={deleteTarget}
      />
      <ObjectMutationDialog
        mutation={objectMutation}
        newName={objectMutationName}
        onCancel={() => setObjectMutation(null)}
        onConfirm={() => void confirmObjectMutation()}
        onNameChange={changeObjectMutationName}
        onOpenChange={(open) => !open && setObjectMutation(null)}
      />
    </div>
  );
}
