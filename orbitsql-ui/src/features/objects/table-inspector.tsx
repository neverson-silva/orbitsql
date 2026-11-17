import { useSearch } from "@tanstack/react-router";
import { type ReactNode, useMemo, useState } from "react";
import { Icon } from "../../components/ui/icons";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "../../components/ui/input-group";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../../components/ui/table";
import {
  Button,
  SegmentedTabs,
  StatusPill,
  Surface,
} from "../../components/ui/ui";
import { useCatalogStore } from "../../stores/catalog.store";
import { refreshConnectionCatalog } from "../connections/connection-ipc";
import type { CatalogTable } from "../sql-intelligence/catalog";

const viewTabs = ["Properties", "Data", "Diagram"];

export function TableInspectorScreen() {
  const search = useSearch({ from: "/objects/table-inspector" });
  const snapshot = useCatalogStore((state) => state.snapshot);
  const [view, setView] = useState(
    search.view === "data"
      ? "Data"
      : search.view === "diagram"
        ? "Diagram"
        : "Properties",
  );
  const [filter, setFilter] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const table = useMemo<CatalogTable | undefined>(() => {
    const table = snapshot.tables.find((item) => item.id === search.tableId);
    if (table) return table;
    const view = [...snapshot.views, ...snapshot.materializedViews].find(
      (item) => item.id === search.tableId,
    );
    return view
      ? {
          ...view,
          foreign: false,
          temporary: false,
          foreignKeys: [],
          uniqueConstraints: [],
          checkConstraints: [],
          indexes: [],
        }
      : undefined;
  }, [
    search.tableId,
    snapshot.materializedViews,
    snapshot.tables,
    snapshot.views,
  ]);
  const setSnapshot = useCatalogStore((state) => state.setSnapshot);
  const columns = useMemo(
    () =>
      (table?.columns ?? []).filter((column) => {
        const value = `${column.name.raw} ${column.type.name}`.toLowerCase();
        return value.includes(filter.toLowerCase());
      }),
    [filter, table],
  );

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
      <EmptyObjectState
        title="Table metadata unavailable"
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
              Live metadata from the connected {snapshot.dialect} catalog.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <StatusPill tone="success">Loaded</StatusPill>
            <Button
              variant="secondary"
              icon="refresh"
              onClick={() => void refreshTableCatalog()}
            >
              {refreshing ? "Refreshing" : "Refresh catalog"}
            </Button>
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-outline-variant/70 px-4 py-3">
          <SegmentedTabs
            options={viewTabs.map((tab) => ({
              id: tab,
              label: tab,
              icon:
                tab === "Properties"
                  ? "list"
                  : tab === "Data"
                    ? "grid"
                    : "diagram",
            }))}
            value={view}
            onChange={setView}
          />
          {view === "Properties" ? (
            <InputGroup className="h-8 w-auto">
              <InputGroupAddon align="inline-start">
                <Icon name="search" size={12} />
              </InputGroupAddon>
              <InputGroupInput
                aria-label="Filter columns"
                className="h-7 w-40 flex-none px-0 text-label-caps"
                onChange={(event) => setFilter(event.currentTarget.value)}
                placeholder="Filter columns"
                value={filter}
              />
            </InputGroup>
          ) : null}
        </div>
        {view === "Properties" ? (
          <Properties table={table} columns={columns} />
        ) : (
          <EmptyObjectState
            title={`${view} is not loaded`}
            description="Open a live query or refresh the catalog to load this view."
            compact
          />
        )}
      </Surface>
    </div>
  );
}

function Properties({
  table,
  columns,
}: {
  table: {
    columns: readonly {
      id: string;
      ordinal: number;
      name: { raw: string };
      type: { name: string };
      nullable: boolean;
      generated: boolean;
      identity: boolean;
      defaultValue?: string;
      primaryKey: boolean;
    }[];
    indexes: readonly {
      id: string;
      name: { raw: string };
      unique: boolean;
      primary: boolean;
    }[];
    primaryKey?: { name: { raw: string }; columnIds: readonly string[] };
    foreignKeys: readonly { name: { raw: string }; targetTableId: string }[];
    checkConstraints: readonly {
      name: { raw: string };
      expression?: string;
    }[];
    estimatedRows?: number;
  };
  columns: readonly {
    id: string;
    ordinal: number;
    name: { raw: string };
    type: { name: string };
    nullable: boolean;
    generated: boolean;
    identity: boolean;
    defaultValue?: string;
    primaryKey: boolean;
  }[];
}) {
  return (
    <div className="grid gap-4 p-4 lg:grid-cols-[1.5fr_0.8fr]">
      <Surface className="overflow-hidden">
        <div className="border-b border-surface-divider bg-surface-subtle px-4 py-3">
          <h2 className="text-body-md font-semibold">Columns</h2>
          <p className="mt-1 text-label-caps text-on-surface-variant">
            {columns.length} of {table.columns.length} live columns
          </p>
        </div>
        <div className="overflow-x-auto">
          <Table className="w-full min-w-[720px] text-left font-mono text-label-caps">
            <TableHeader className="bg-surface-subtle text-label-caps uppercase ds-tracking-caps text-on-surface-variant">
              <TableRow>
                {["Column", "Data type", "Nullable", "Default", "Role"].map(
                  (heading) => (
                    <TableHead
                      className="border-b border-outline-variant/60 px-4 py-2.5 font-medium"
                      key={heading}
                    >
                      {heading}
                    </TableHead>
                  ),
                )}
              </TableRow>
            </TableHeader>
            <TableBody>
              {columns.map((column) => (
                <TableRow
                  className="border-b border-outline-variant/50"
                  key={column.id}
                >
                  <TableCell className="px-4 py-2.5 text-primary">
                    {column.name.raw}
                  </TableCell>
                  <TableCell className="px-4 py-2.5 text-on-surface">
                    {column.type.name}
                  </TableCell>
                  <TableCell className="px-4 py-2.5 text-on-surface-variant">
                    {column.nullable ? "YES" : "NO"}
                  </TableCell>
                  <TableCell className="max-w-56 truncate px-4 py-2.5 text-on-surface-variant">
                    {column.defaultValue ?? "—"}
                  </TableCell>
                  <TableCell className="px-4 py-2.5">
                    {column.primaryKey ? (
                      <StatusPill tone="primary">PK</StatusPill>
                    ) : column.identity ? (
                      "IDENTITY"
                    ) : column.generated ? (
                      "GENERATED"
                    ) : (
                      "—"
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
        <div className="border-t border-outline-variant/60 px-4 py-2 font-mono text-label-caps text-on-surface-variant">
          {columns.length} columns shown from the live catalog
        </div>
      </Surface>
      <div className="space-y-3">
        <DefinitionCard title="Keys & Indexes" icon="key">
          {table.primaryKey ? (
            <p className="text-label-caps text-primary">
              PK · {table.primaryKey.name.raw}
            </p>
          ) : (
            <p className="text-label-caps text-on-surface-variant">
              No primary key reported
            </p>
          )}
          <div className="mt-3 space-y-2 font-mono text-label-caps text-on-surface-variant">
            {table.indexes.map((index) => (
              <div className="flex justify-between gap-2" key={index.id}>
                <span className="truncate text-primary">{index.name.raw}</span>
                <span>
                  {index.primary
                    ? "PRIMARY"
                    : index.unique
                      ? "UNIQUE"
                      : "INDEX"}
                </span>
              </div>
            ))}
          </div>
        </DefinitionCard>
        <DefinitionCard title="Relations & Checks" icon="link">
          <div className="space-y-2 text-label-caps text-on-surface-variant">
            {table.foreignKeys.map((key) => (
              <div key={key.name.raw}>
                <span className="text-primary">{key.name.raw}</span>
                <span className="ml-2">→ {key.targetTableId}</span>
              </div>
            ))}
            {table.checkConstraints.map((check) => (
              <div key={check.name.raw}>
                <span className="text-primary">{check.name.raw}</span>
                <span className="ml-2">{check.expression ?? "CHECK"}</span>
              </div>
            ))}
            {!table.foreignKeys.length && !table.checkConstraints.length ? (
              <span>No relations or checks reported</span>
            ) : null}
          </div>
        </DefinitionCard>
        <DefinitionCard title="Catalog facts" icon="list">
          <div className="space-y-2 font-mono text-label-caps text-on-surface-variant">
            <div className="flex justify-between">
              <span>Estimated rows</span>
              <span className="text-on-surface">
                {table.estimatedRows?.toLocaleString() ?? "—"}
              </span>
            </div>
            <div className="flex justify-between">
              <span>Indexes</span>
              <span className="text-on-surface">{table.indexes.length}</span>
            </div>
            <div className="flex justify-between">
              <span>Columns</span>
              <span className="text-on-surface">{table.columns.length}</span>
            </div>
          </div>
        </DefinitionCard>
      </div>
    </div>
  );
}

function DefinitionCard({
  title,
  icon,
  children,
}: {
  title: string;
  icon: "key" | "link" | "list";
  children: ReactNode;
}) {
  return (
    <Surface tone="subtle" className="p-4">
      <div className="mb-3 flex items-center gap-2 text-body-sm font-semibold">
        <Icon name={icon} size={14} className="text-primary" />
        {title}
      </div>
      {children}
    </Surface>
  );
}

function EmptyObjectState({
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
          <Icon name="table" size={28} className="mb-3 text-primary" />
          <h1 className="text-body-md font-semibold">{title}</h1>
          <p className="mt-1 max-w-md text-body-sm text-on-surface-variant">
            {description}
          </p>
        </div>
      </Surface>
    </div>
  );
}
