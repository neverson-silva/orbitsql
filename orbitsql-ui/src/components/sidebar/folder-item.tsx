import type {
  ExplorerConnection,
  ExplorerDragItem,
  ExplorerDropTarget,
  ExplorerFolder,
} from "../../stores/connections.store";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuGroup,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "../ui/context-menu";
import { Icon } from "../ui/icons";
import { cn } from "../ui/ui";
import { ConnectionItem } from "./connection-item";
import type { SidebarActions } from "./sidebar.types";

function matches(
  source: ExplorerDragItem | ExplorerDropTarget | null,
  target: ExplorerDragItem,
) {
  return Boolean(
    source &&
      source.kind === target.kind &&
      source.id === target.id &&
      source.parentFolderId === target.parentFolderId,
  );
}

function connectionCount(
  folder: ExplorerFolder,
  folders: Record<string, ExplorerFolder>,
): number {
  return (
    folder.connectionIds.length +
    folder.folderIds.reduce((count, folderId) => {
      const child = folders[folderId];
      return count + (child ? connectionCount(child, folders) : 0);
    }, 0)
  );
}

export function FolderItem({
  folder,
  connections,
  folders,
  activeConnectionId,
  refreshingIds,
  dragging,
  dropTarget,
  actions,
  selectedSchema,
}: {
  folder: ExplorerFolder;
  connections: Record<string, ExplorerConnection>;
  folders: Record<string, ExplorerFolder>;
  activeConnectionId: string | null;
  refreshingIds: Set<string>;
  dragging: ExplorerDragItem | null;
  dropTarget: ExplorerDropTarget | null;
  actions: SidebarActions;
  selectedSchema?: string;
}) {
  const item: ExplorerDragItem = {
    kind: "folder",
    id: folder.id,
    parentFolderId: folder.parentFolderId,
  };
  const highlighted = matches(dropTarget, item);
  const insertBefore =
    highlighted &&
    dropTarget?.mode === "insert" &&
    dropTarget.position === "before";
  const insertAfter =
    highlighted &&
    dropTarget?.mode === "insert" &&
    dropTarget.position === "after";

  return (
    <div className="min-w-0 space-y-0.5">
      <li
        className="relative min-w-0 cursor-grab active:cursor-grabbing"
        onPointerDown={(event) => actions.startPointerDrag(event, item)}
        data-sidebar-dnd-kind={item.kind}
        data-sidebar-dnd-id={item.id}
        data-sidebar-dnd-parent-folder-id={folder.parentFolderId ?? ""}
      >
        <span
          aria-hidden="true"
          className={cn(
            "pointer-events-none absolute inset-x-0 -top-px z-10 h-0.5 bg-primary transition-[opacity,transform] duration-100 ease-out",
            insertBefore ? "scale-x-100 opacity-100" : "scale-x-95 opacity-0",
          )}
        />
        <span
          aria-hidden="true"
          className={cn(
            "pointer-events-none absolute inset-x-0 -bottom-px z-10 h-0.5 bg-primary transition-[opacity,transform] duration-100 ease-out",
            insertAfter ? "scale-x-100 opacity-100" : "scale-x-95 opacity-0",
          )}
        />
        <ContextMenu>
          <ContextMenuTrigger className="block min-w-0">
            <button
              aria-expanded={folder.expanded}
              className={cn(
                "flex min-h-9 w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left transition-[background-color,box-shadow,opacity] duration-150 ease-out hover:bg-app-chrome-subtle focus-visible:bg-app-chrome-subtle focus-visible:outline-none",
                matches(dragging, item) && "opacity-40",
                highlighted &&
                  dropTarget?.mode !== "insert" &&
                  "bg-selected-surface ring-1 ring-inset ring-primary/30",
              )}
              onClick={() => {
                if (actions.consumePointerClick()) return;
                actions.toggleFolder(folder.id);
              }}
              type="button"
            >
              <Icon
                className="shrink-0 text-on-surface-variant"
                name={folder.expanded ? "chevron-down" : "chevron-right"}
                size={11}
              />
              <Icon
                className="shrink-0"
                name="folder"
                size={13}
                style={{ color: folder.color }}
              />
              <span className="min-w-0 flex-1 truncate text-label-md font-medium text-on-surface">
                {folder.name}
              </span>
              <span className="shrink-0 font-mono text-[10px] text-on-surface-variant">
                {connectionCount(folder, folders)}
              </span>
            </button>
          </ContextMenuTrigger>
          <ContextMenuContent className="w-44 p-1" side="right">
            <ContextMenuGroup>
              <ContextMenuLabel className="max-w-40 truncate">
                {folder.name}
              </ContextMenuLabel>
              <ContextMenuItem onClick={() => actions.openFolderEditor(folder)}>
                <Icon name="settings" size={14} />
                Edit folder
              </ContextMenuItem>
              <ContextMenuItem onClick={() => actions.openFolderEditor(folder)}>
                <Icon name="pencil" size={14} />
                Rename
              </ContextMenuItem>
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuItem
              onClick={() =>
                actions.requestDelete({ kind: "folder", id: folder.id })
              }
              variant="destructive"
            >
              <Icon name="trash" size={14} />
              Delete folder
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
      </li>
      {folder.expanded ? (
        <div className="min-w-0 border-s border-outline-variant/70 ps-2">
          {folder.folderIds.map((folderId) => {
            const childFolder = folders[folderId];
            return childFolder ? (
              <FolderItem
                actions={actions}
                activeConnectionId={activeConnectionId}
                connections={connections}
                folders={folders}
                dragging={dragging}
                dropTarget={dropTarget}
                folder={childFolder}
                key={childFolder.id}
                refreshingIds={refreshingIds}
                selectedSchema={selectedSchema}
              />
            ) : null;
          })}
          {folder.connectionIds.map((connectionId) => {
            const connection = connections[connectionId];
            return connection ? (
              <ConnectionItem
                actions={actions}
                activeConnectionId={activeConnectionId}
                connection={connection}
                dragging={dragging}
                dropTarget={dropTarget}
                key={connection.id}
                parentFolderId={folder.id}
                refreshing={refreshingIds.has(connection.id)}
                selectedSchema={selectedSchema}
              />
            ) : null;
          })}
          <div
            aria-label={`Drop connection into ${folder.name}`}
            className={cn(
              "rounded-md px-2 py-1 text-[10px] text-on-surface-variant/70",
              highlighted && "bg-selected-surface text-primary",
            )}
            onDragOver={(event) => actions.dragOver(event, item)}
            onDrop={(event) => actions.drop(event, item)}
            role="listbox"
            data-sidebar-dnd-kind={item.kind}
            data-sidebar-dnd-id={item.id}
            data-sidebar-dnd-parent-folder-id={folder.parentFolderId ?? ""}
          >
            {folder.connectionIds.length === 0 && folder.folderIds.length === 0
              ? "Drop items here"
              : "Drop to add item"}
          </div>
        </div>
      ) : null}
    </div>
  );
}
