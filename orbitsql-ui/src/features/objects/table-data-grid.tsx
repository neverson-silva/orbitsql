import { useSearch } from "@tanstack/react-router";
import { useState } from "react";
import { Icon } from "../../components/ui/icons";
import {
  Button,
  SegmentedTabs,
  StatusPill,
  Surface,
} from "../../components/ui/ui";
import { useCatalogStore } from "../../stores/catalog.store";
import { refreshConnectionCatalog } from "../connections/connection-ipc";

const viewTabs = ["Data", "Properties", "Diagram"];

export function TableDataGridScreen() {
  const search = useSearch({ from: "/objects/table-data" });
  const snapshot = useCatalogStore((state) => state.snapshot);
  const [view, setView] = useState(
    search.view === "properties"
      ? "Properties"
      : search.view === "diagram"
        ? "Diagram"
        : "Data",
  );
  const [refreshing, setRefreshing] = useState(false);
  const table =
    snapshot.tables.find((item) => item.id === search.tableId) ??
    [...snapshot.views, ...snapshot.materializedViews].find(
      (item) => item.id === search.tableId,
    );
  const setSnapshot = useCatalogStore((state) => state.setSnapshot);

  async function refreshTableCatalog() {
    if (!search.connectionId || refreshing) return;
    setRefreshing(true);
    try {
      const refreshed = await refreshConnectionCatalog(search.connectionId);
      setSnapshot(refreshed);
    } finally {
      setRefreshing(false);
    }
  }

  if (!table) {
    return (
      <EmptyState
        title="Table data unavailable"
        description="Refresh the connection explorer and open a table from the live catalog."
      />
    );
  }

  return (
    <div className="mx-auto flex max-w-[1540px] flex-col gap-3 p-5 lg:p-7">
      <div className="flex min-w-0 items-center gap-2 border-b border-outline-variant/70 pb-2 font-mono text-label-caps text-on-surface-variant">
        <Icon name="database" size={12} />
        <span>{table.databaseId}</span>
        <Icon name="chevron-right" size={11} />
        <span>{table.schemaId}</span>
        <Icon name="chevron-right" size={11} />
        <span className="text-on-surface">{table.name.raw}</span>
        <StatusPill tone="neutral">TABLE</StatusPill>
      </div>
      <Surface className="overflow-hidden">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-outline-variant/70 px-4 py-3">
          <div>
            <h1 className="text-headline-sm font-semibold">{table.name.raw}</h1>
            <p className="mt-1 text-body-sm text-on-surface-variant">
              Real catalog metadata from the connected {snapshot.dialect}{" "}
              database.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <StatusPill tone="success">Connected</StatusPill>
            <Button
              variant="secondary"
              icon="refresh"
              onClick={() => void refreshTableCatalog()}
            >
              {refreshing ? "Refreshing" : "Refresh catalog"}
            </Button>
          </div>
        </div>
        <div className="border-b border-outline-variant/70 px-4 py-3">
          <SegmentedTabs
            options={viewTabs.map((tab) => ({
              id: tab,
              label: tab,
              icon:
                tab === "Data"
                  ? "grid"
                  : tab === "Properties"
                    ? "list"
                    : "diagram",
            }))}
            value={view}
            onChange={setView}
          />
        </div>
        {view === "Properties" ? (
          <div className="overflow-x-auto p-4">
            <table className="w-full min-w-[620px] text-left font-mono text-label-caps">
              <thead className="bg-surface-subtle text-on-surface-variant">
                <tr>
                  {["Column", "Type", "Nullable", "Default"].map((heading) => (
                    <th
                      className="border-b border-outline-variant/60 px-4 py-2.5 font-medium"
                      key={heading}
                    >
                      {heading}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {table.columns.map((column) => (
                  <tr
                    className="border-b border-outline-variant/50"
                    key={column.id}
                  >
                    <td className="px-4 py-2.5 text-primary">
                      {column.name.raw}
                    </td>
                    <td className="px-4 py-2.5 text-on-surface">
                      {column.type.name}
                    </td>
                    <td className="px-4 py-2.5 text-on-surface-variant">
                      {column.nullable ? "YES" : "NO"}
                    </td>
                    <td className="px-4 py-2.5 text-on-surface-variant">
                      {column.defaultValue ?? "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <EmptyState
            compact
            title={
              view === "Data"
                ? "No data query has been run"
                : "Diagram metadata is not loaded"
            }
            description="Run a query from the workspace or load the requested catalog scope to show live data here."
          />
        )}
      </Surface>
    </div>
  );
}

function EmptyState({
  title,
  description,
  compact = false,
}: {
  title: string;
  description: string;
  compact?: boolean;
}) {
  return (
    <div className="mx-auto flex max-w-[900px] flex-col gap-3 p-5 lg:p-7">
      <Surface className={compact ? "p-10" : "p-16"}>
        <div className="flex min-h-40 flex-col items-center justify-center text-center">
          <Icon name="grid" size={28} className="mb-3 text-primary" />
          <h1 className="text-body-md font-semibold">{title}</h1>
          <p className="mt-1 max-w-md text-body-sm text-on-surface-variant">
            {description}
          </p>
        </div>
      </Surface>
    </div>
  );
}
