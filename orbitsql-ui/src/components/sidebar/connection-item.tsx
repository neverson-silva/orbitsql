import {
  DatabaseBrandIcon,
  databaseBrandForEngine,
} from "../../features/connections/database-brand-icon";
import { profileColorHex } from "../../features/connections/profile-style";
import type {
  ExplorerConnection,
  ExplorerDragItem,
  ExplorerDropTarget,
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
import { ConnectionStatus } from "./connection-status";
import { ConnectionTree } from "./connection-tree";
import type { SidebarActions } from "./sidebar.types";
import { sidebarEngineIcons } from "./sidebar-icons";

function sameItem(
  source: ExplorerDragItem | ExplorerDropTarget | null,
  item: ExplorerDragItem,
) {
  return source
    ? source.kind === item.kind &&
        source.id === item.id &&
        source.parentFolderId === item.parentFolderId
    : false;
}

export function ConnectionItem({
  connection,
  parentFolderId,
  activeConnectionId,
  refreshing,
  dragging,
  dropTarget,
  actions,
  selectedSchema,
}: {
  connection: ExplorerConnection;
  parentFolderId: string | null;
  activeConnectionId: string | null;
  refreshing: boolean;
  dragging: ExplorerDragItem | null;
  dropTarget: ExplorerDropTarget | null;
  actions: SidebarActions;
  selectedSchema?: string;
}) {
  const item: ExplorerDragItem = {
    kind: "connection",
    id: connection.id,
    parentFolderId,
  };
  const selected = activeConnectionId === connection.id;
  const insertBefore =
    sameItem(dropTarget, item) &&
    dropTarget?.mode === "insert" &&
    dropTarget.position === "before";
  const insertAfter =
    sameItem(dropTarget, item) &&
    dropTarget?.mode === "insert" &&
    dropTarget.position === "after";
  const connected =
    connection.status === "connected" || connection.status === "ready";
  const profileName = connection.detail.split(" • ")[0] ?? connection.detail;
  const engineIcon = sidebarEngineIcons[connection.engine] ?? "database";
  const databaseBrand = databaseBrandForEngine(connection.engine);

  return (
    <div className="min-w-0" data-slot="sidebar-connection-item">
      <li
        className="relative min-w-0 cursor-grab active:cursor-grabbing"
        onPointerDown={(event) => actions.startPointerDrag(event, item)}
        data-sidebar-dnd-kind={item.kind}
        data-sidebar-dnd-id={item.id}
        data-sidebar-dnd-parent-folder-id={item.parentFolderId ?? ""}
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
              aria-expanded={connection.expanded}
              className={cn(
                "flex min-h-14 w-full min-w-0 items-center gap-2 rounded-md border border-transparent px-2.5 py-2 text-left transition-[background-color,border-color,box-shadow,opacity] duration-150 ease-out hover:bg-app-chrome-subtle focus-visible:bg-app-chrome-subtle focus-visible:outline-none",
                selected &&
                  "border-primary/30 bg-selected-surface ring-1 ring-inset ring-primary/20",
                sameItem(dragging, item) && "opacity-40",
                sameItem(dropTarget, item) &&
                  dropTarget?.mode !== "insert" &&
                  "bg-selected-surface ring-1 ring-inset ring-primary/30",
              )}
              onClick={() => {
                if (actions.consumePointerClick()) return;
                actions.selectConnection(connection.id);
                actions.toggleConnection(connection.id);
              }}
              type="button"
            >
              <Icon
                className="shrink-0 text-on-surface-variant"
                name={connection.expanded ? "chevron-down" : "chevron-right"}
                size={11}
              />
              {databaseBrand ? (
                <DatabaseBrandIcon
                  className="shrink-0 text-primary"
                  brand={databaseBrand}
                  size={16}
                />
              ) : (
                <Icon
                  className="shrink-0 text-on-surface-variant"
                  name={engineIcon}
                  size={14}
                />
              )}
              <span className="flex min-w-0 flex-1 flex-col justify-center">
                <span className="truncate text-label-md font-medium text-on-surface">
                  {connection.name}
                </span>
              </span>
              <span className="flex shrink-0 items-center gap-2">
                <span
                  aria-label={`Profile: ${profileName}`}
                  className="size-2 rounded-full"
                  role="img"
                  style={{
                    backgroundColor: profileColorHex(connection.profileColor),
                  }}
                  title={profileName}
                />
                <ConnectionStatus
                  refreshing={refreshing}
                  status={connection.status}
                />
              </span>
            </button>
          </ContextMenuTrigger>
          <ContextMenuContent className="w-52 p-1" side="right">
            <ContextMenuGroup>
              <ContextMenuLabel className="max-w-48 truncate">
                {connection.name}
              </ContextMenuLabel>
              <ContextMenuItem
                onClick={() => actions.toggleRemoteConnection(connection)}
              >
                <Icon name={connected ? "close" : "link"} size={14} />
                {connected ? "Disconnect" : "Connect"}
              </ContextMenuItem>
              {connected ? (
                <>
                  <ContextMenuItem
                    onClick={() => actions.refreshConnection(connection.id)}
                  >
                    <Icon name="refresh" size={14} />
                    Refresh introspection
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() => actions.openQueryScript(connection.id)}
                  >
                    <Icon name="sql-file" size={14} />
                    Open new query script
                  </ContextMenuItem>
                </>
              ) : null}
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuGroup>
              <ContextMenuItem
                onClick={() => actions.openConnectionEditor(connection, "edit")}
              >
                <Icon name="settings" size={14} />
                Edit connection
              </ContextMenuItem>
              <ContextMenuItem
                onClick={() =>
                  actions.openConnectionEditor(connection, "rename")
                }
              >
                <Icon name="pencil" size={14} />
                Rename
              </ContextMenuItem>
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuItem
              onClick={() =>
                actions.requestDelete({ kind: "connection", id: connection.id })
              }
              variant="destructive"
            >
              <Icon name="trash" size={14} />
              Delete connection
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
      </li>
      <ConnectionTree
        actions={actions}
        connection={connection}
        selectedSchema={selectedSchema}
      />
    </div>
  );
}
