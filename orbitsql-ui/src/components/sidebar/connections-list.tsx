import type { DragEvent } from "react";
import type {
  ExplorerConnection,
  ExplorerDragItem,
  ExplorerDropTarget,
  ExplorerFolder,
  ExplorerRootItem,
} from "../../stores/connections.store";
import { ConnectionItem } from "./connection-item";
import { FolderItem } from "./folder-item";
import type { SidebarActions } from "./sidebar.types";

export function ConnectionsList({
  rootOrder,
  connections,
  folders,
  activeConnectionId,
  refreshingIds,
  dragging,
  dropTarget,
  actions,
  selectedSchema,
}: {
  rootOrder: ExplorerRootItem[];
  connections: Record<string, ExplorerConnection>;
  folders: Record<string, ExplorerFolder>;
  activeConnectionId: string | null;
  refreshingIds: Set<string>;
  dragging: ExplorerDragItem | null;
  dropTarget: ExplorerDropTarget | null;
  actions: SidebarActions;
  selectedSchema?: string;
}) {
  function targetFromEvent(
    event: DragEvent<HTMLElement>,
  ): ExplorerDropTarget | null {
    const element = (event.target as Element).closest<HTMLElement>(
      "[data-sidebar-dnd-kind]",
    );
    if (!element) return null;
    const kind = element.dataset.sidebarDndKind;
    const id = element.dataset.sidebarDndId;
    if ((kind !== "connection" && kind !== "folder") || !id) return null;
    return {
      kind,
      id,
      parentFolderId: element.dataset.sidebarDndParentFolderId || null,
    };
  }

  function handleDragOver(event: DragEvent<HTMLDivElement>) {
    const target = targetFromEvent(event);
    if (target) actions.dragOver(event, target);
  }

  function handleDrop(event: DragEvent<HTMLDivElement>) {
    const target = targetFromEvent(event);
    if (target) actions.drop(event, target);
  }

  return (
    <div
      className="flex h-full min-h-0 min-w-0 flex-1 flex-col"
      data-slot="sidebar-connection-list"
    >
      <section
        className="min-h-0 min-w-0 flex-1 overflow-y-auto px-2 py-2"
        onDragOver={handleDragOver}
        onDrop={handleDrop}
        aria-label="Connection drop area"
      >
        <nav aria-label="Database connections" className="min-w-0 space-y-2">
          {rootOrder.map((item) => {
            if (item.kind === "folder") {
              const folder = folders[item.id];
              return folder ? (
                <FolderItem
                  actions={actions}
                  activeConnectionId={activeConnectionId}
                  connections={connections}
                  folders={folders}
                  dragging={dragging}
                  dropTarget={dropTarget}
                  folder={folder}
                  key={folder.id}
                  refreshingIds={refreshingIds}
                  selectedSchema={selectedSchema}
                />
              ) : null;
            }
            const connection = connections[item.id];
            return connection ? (
              <ConnectionItem
                actions={actions}
                activeConnectionId={activeConnectionId}
                connection={connection}
                dragging={dragging}
                dropTarget={dropTarget}
                key={connection.id}
                parentFolderId={null}
                refreshing={refreshingIds.has(connection.id)}
                selectedSchema={selectedSchema}
              />
            ) : null;
          })}
          {rootOrder.length === 0 ? (
            <div className="rounded-md border border-dashed border-outline-variant/70 px-3 py-4 text-center text-label-caps text-on-surface-variant">
              No connections yet
            </div>
          ) : null}
        </nav>
      </section>
    </div>
  );
}
