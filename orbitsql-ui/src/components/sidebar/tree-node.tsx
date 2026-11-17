import { Link } from "@tanstack/react-router";
import type {
  ExplorerChild,
  ExplorerConnection,
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
import type { SidebarActions } from "./sidebar.types";
import { getDatabaseObjectVisual } from "./sidebar-icons";

function supportsContextMenu(child: ExplorerChild): boolean {
  return child.kind !== "metadata";
}

export function TreeNode({
  connection,
  child,
  depth,
  ancestors,
  actions,
  selectedSchema,
}: {
  connection: ExplorerConnection;
  child: ExplorerChild;
  depth: number;
  ancestors: ReadonlySet<string>;
  actions: SidebarActions;
  selectedSchema?: string;
}) {
  if (ancestors.has(child.id)) return null;
  const nextAncestors = new Set(ancestors).add(child.id);
  const nested = connection.children.filter(
    (candidate) =>
      candidate.parentId === child.id &&
      (child.kind !== "schema" || candidate.kind === "group"),
  );
  const rowStyle = { paddingInlineStart: 6 + depth * 14 };
  const toggle = () => {
    if (child.kind === "database") actions.expandDatabase(connection.id, child);
    if (child.kind === "schema") actions.expandSchema(connection.id, child);
    if (child.kind === "group") actions.toggleChild(connection.id, child.id);
    if (child.kind === "table") actions.expandTable(connection.id, child);
  };
  const visual = getDatabaseObjectVisual(child);
  const ObjectIcon = visual.icon;
  const schemaSelected =
    child.kind === "schema" && child.name === selectedSchema;
  const hasContextMenu = supportsContextMenu(child);
  const canMutate = child.kind !== "group";
  const expandable =
    child.kind === "database" ||
    child.kind === "schema" ||
    child.kind === "group" ||
    child.kind === "table";

  const row = (
    <div className="flex min-w-0 items-center gap-1.5 py-0.5" style={rowStyle}>
      {expandable ? (
        <button
          aria-expanded={child.expanded}
          aria-label={
            child.loading
              ? `Loading ${child.name}`
              : `${child.expanded ? "Collapse" : "Expand"} ${child.name}`
          }
          className="flex size-5 shrink-0 items-center justify-center rounded text-on-surface-variant hover:bg-app-chrome-subtle hover:text-on-surface focus-visible:outline-none"
          onClick={toggle}
          disabled={child.loading}
          type="button"
        >
          <Icon
            className={child.loading ? "animate-spin text-primary" : undefined}
            name={
              child.loading
                ? "spinner"
                : child.expanded
                  ? "chevron-down"
                  : "chevron-right"
            }
            size={10}
          />
        </button>
      ) : (
        <span className="size-5 shrink-0" />
      )}
      {child.kind === "table" ? (
        <Link
          className="flex min-w-0 flex-1 items-center gap-1.5 rounded px-1.5 py-1 text-label-caps text-on-surface-variant hover:bg-app-chrome-subtle hover:text-on-surface"
          to={
            child.name === "users"
              ? "/schema"
              : child.name === "billing_events"
                ? "/objects/table-data"
                : "/objects/table-inspector"
          }
          search={{
            connectionId: connection.id,
            tableId: child.remoteId ?? child.id,
            view: "properties",
          }}
        >
          <ObjectIcon
            aria-hidden="true"
            color={visual.color}
            opacity={visual.opacity}
            size={visual.size}
            weight={visual.weight}
          />
          <span className="min-w-0 truncate">{child.name}</span>
        </Link>
      ) : (
        <button
          aria-expanded={expandable ? child.expanded : undefined}
          aria-current={schemaSelected ? "location" : undefined}
          className={`flex min-w-0 flex-1 items-center gap-1.5 rounded px-1.5 py-1 text-left text-label-caps text-on-surface-variant hover:bg-app-chrome-subtle hover:text-on-surface focus-visible:outline-none ${schemaSelected ? "bg-selected-surface text-primary" : ""}`}
          onClick={expandable ? toggle : undefined}
          type="button"
        >
          <ObjectIcon
            aria-hidden="true"
            color={visual.color}
            opacity={visual.opacity}
            size={visual.size}
            weight={visual.weight}
          />
          <span className="min-w-0 truncate">{child.name}</span>
        </button>
      )}
    </div>
  );

  return (
    <div className="min-w-0">
      {hasContextMenu ? (
        <ContextMenu>
          <ContextMenuTrigger className="block min-w-0">
            {row}
          </ContextMenuTrigger>
          <ContextMenuContent className="w-56 p-1" side="right">
            <ContextMenuGroup>
              <ContextMenuLabel className="max-w-52 truncate">
                {child.name}
              </ContextMenuLabel>
              {child.kind === "table" || child.kind === "view" ? (
                <>
                  <ContextMenuItem
                    onClick={() =>
                      actions.openObjectView(connection.id, child, "properties")
                    }
                  >
                    <Icon name="list" size={14} />
                    View Table · Properties
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() =>
                      actions.openObjectView(connection.id, child, "data")
                    }
                  >
                    <Icon name="grid" size={14} />
                    View Data
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() =>
                      actions.openObjectView(connection.id, child, "diagram")
                    }
                  >
                    <Icon name="diagram" size={14} />
                    View Diagram
                  </ContextMenuItem>
                  <ContextMenuSeparator />
                  <ContextMenuItem
                    onClick={() =>
                      actions.exportObjectData(connection.id, child)
                    }
                  >
                    <Icon name="download" size={14} />
                    Export Data
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() =>
                      actions.importObjectData(connection.id, child)
                    }
                  >
                    <Icon name="plus" size={14} />
                    Import Data
                  </ContextMenuItem>
                </>
              ) : null}
              {canMutate ? (
                <>
                  <ContextMenuItem
                    onClick={() => actions.copyObjectName(child)}
                  >
                    <Icon name="copy" size={14} />
                    Copy name
                  </ContextMenuItem>
                  <ContextMenuSeparator />
                  <ContextMenuItem
                    onClick={() =>
                      actions.requestObjectMutation(
                        connection.id,
                        child,
                        "rename",
                      )
                    }
                  >
                    <Icon name="pencil" size={14} />
                    Rename
                  </ContextMenuItem>
                  <ContextMenuItem
                    onClick={() =>
                      actions.requestObjectMutation(
                        connection.id,
                        child,
                        "delete",
                      )
                    }
                    variant="destructive"
                  >
                    <Icon name="trash" size={14} />
                    Delete
                  </ContextMenuItem>
                  <ContextMenuSeparator />
                </>
              ) : null}
              <ContextMenuItem
                onClick={() => actions.refreshObject(connection.id, child)}
              >
                <Icon name="refresh" size={14} />
                Refresh object
              </ContextMenuItem>
              {child.kind === "table" ||
              child.kind === "view" ||
              child.kind === "database" ||
              child.kind === "schema" ||
              child.kind === "user" ||
              child.kind === "group" ? (
                <ContextMenuItem
                  onClick={() => actions.openQueryScript(connection.id, child)}
                >
                  <Icon name="sql-file" size={14} />
                  Open new query script
                </ContextMenuItem>
              ) : null}
            </ContextMenuGroup>
          </ContextMenuContent>
        </ContextMenu>
      ) : (
        row
      )}
      {child.expanded
        ? nested.map((nestedChild) => (
            <TreeNode
              actions={actions}
              ancestors={nextAncestors}
              child={nestedChild}
              connection={connection}
              depth={depth + 1}
              key={nestedChild.id}
            />
          ))
        : null}
    </div>
  );
}
