import { createFileRoute } from "@tanstack/react-router";
import { TableDataGridScreen } from "../features/objects/table-data-grid";

export const Route = createFileRoute("/objects/table-data")({
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
  component: TableDataGridScreen,
});
