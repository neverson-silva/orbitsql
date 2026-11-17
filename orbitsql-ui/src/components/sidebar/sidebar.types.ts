import type { DragEvent, PointerEvent } from "react";
import type {
  ExplorerChild,
  ExplorerConnection,
  ExplorerDragItem,
  ExplorerDropTarget,
  ExplorerFolder,
} from "../../stores/connections.store";

export type SidebarEditMode = "rename" | "edit";

export type SidebarDeleteTarget =
  | { kind: "connection"; id: string }
  | { kind: "folder"; id: string };

export type SidebarObjectMutation = {
  connectionId: string;
  child: ExplorerChild;
  operation: "delete" | "rename";
  sql: string;
};

export type SidebarActions = {
  toggleConnection: (connectionId: string) => void;
  toggleFolder: (folderId: string) => void;
  startDrag: (event: DragEvent<HTMLElement>, item: ExplorerDragItem) => void;
  startPointerDrag: (
    event: PointerEvent<HTMLElement>,
    item: ExplorerDragItem,
  ) => void;
  dragOver: (event: DragEvent<HTMLElement>, target: ExplorerDropTarget) => void;
  drop: (event: DragEvent<HTMLElement>, target: ExplorerDropTarget) => void;
  clearDrag: () => void;
  consumePointerClick: () => boolean;
  expandDatabase: (connectionId: string, child: ExplorerChild) => void;
  expandSchema: (connectionId: string, child: ExplorerChild) => void;
  expandTable: (connectionId: string, child: ExplorerChild) => void;
  toggleChild: (connectionId: string, childId: string) => void;
  selectConnection: (connectionId: string) => void;
  toggleRemoteConnection: (connection: ExplorerConnection) => void;
  refreshConnection: (connectionId: string) => void;
  refreshObject: (connectionId: string, child: ExplorerChild) => void;
  openQueryScript: (connectionId: string, child?: ExplorerChild) => void;
  openObjectView: (
    connectionId: string,
    child: ExplorerChild,
    view: "properties" | "data" | "diagram",
  ) => void;
  copyObjectName: (child: ExplorerChild) => void;
  exportObjectData: (connectionId: string, child: ExplorerChild) => void;
  importObjectData: (connectionId: string, child: ExplorerChild) => void;
  requestObjectMutation: (
    connectionId: string,
    child: ExplorerChild,
    operation: "delete" | "rename",
  ) => void;
  copyConnection: (connection: ExplorerConnection) => void;
  openConnectionEditor: (
    connection: ExplorerConnection,
    mode: SidebarEditMode,
  ) => void;
  requestDelete: (target: SidebarDeleteTarget) => void;
  openFolderEditor: (folder: ExplorerFolder) => void;
};
