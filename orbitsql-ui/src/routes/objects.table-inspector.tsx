import { createFileRoute } from "@tanstack/react-router";
import { TableInspectorScreen } from "../features/objects/table-inspector";

export const Route = createFileRoute("/objects/table-inspector")({
  validateSearch: (search: Record<string, unknown>) => ({
    connectionId:
      typeof search.connectionId === "string" ? search.connectionId : undefined,
    tableId: typeof search.tableId === "string" ? search.tableId : undefined,
    view:
      search.view === "data" ||
      search.view === "diagram" ||
      search.view === "properties"
        ? search.view
        : undefined,
  }),
  component: TableInspectorScreen,
});
