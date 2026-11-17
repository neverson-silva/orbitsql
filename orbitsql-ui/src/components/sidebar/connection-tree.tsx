import type { ExplorerConnection } from "../../stores/connections.store";
import type { SidebarActions } from "./sidebar.types";
import { TreeNode } from "./tree-node";

export function ConnectionTree({
  connection,
  actions,
  selectedSchema,
}: {
  connection: ExplorerConnection;
  actions: SidebarActions;
  selectedSchema?: string;
}) {
  if (!connection.expanded) return null;
  return (
    <div className="min-w-0" data-slot="sidebar-connection-tree">
      {connection.children
        .filter((child) => child.parentId === null)
        .map((child) => (
          <TreeNode
            actions={actions}
            ancestors={new Set()}
            child={child}
            connection={connection}
            depth={0}
            key={child.id}
            selectedSchema={selectedSchema}
          />
        ))}
    </div>
  );
}
