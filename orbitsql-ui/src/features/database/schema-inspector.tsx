import type { ReactNode } from "react";
import { useSearch } from "@tanstack/react-router";
import { useCatalogStore } from "../../stores/catalog.store";
import { Icon } from "../../components/ui/icons";
import { StatusPill, Surface } from "../../components/ui/ui";

export function SchemaInspectorScreen() {
  const search = useSearch({ from: "/schema" });
  const snapshot = useCatalogStore((state) => state.snapshot);
  const schema = snapshot.schemas.find((item) => item.id === search.schemaId);
  const tables = snapshot.tables.filter((table) => table.schemaId === search.schemaId);
  const views = snapshot.views.filter((view) => view.schemaId === search.schemaId);

  if (!schema) {
    return (
      <EmptyState
        title="Schema metadata unavailable"
        description="Refresh the connection explorer and open a schema from the live catalog."
      />
    );
  }

  return (
    <div className="mx-auto flex max-w-[1500px] flex-col gap-4 p-5 lg:p-7">
      <div className="flex items-center gap-2 border-b border-outline-variant/70 pb-3 font-mono text-label-caps text-on-surface-variant">
        <Icon name="database" size={12} />
        <span>{schema.databaseId}</span>
        <Icon name="chevron-right" size={11} />
        <span className="text-on-surface">{schema.name.raw}</span>
        <StatusPill tone="neutral">SCHEMA</StatusPill>
      </div>
      <Surface className="p-5">
        <h1 className="text-headline-sm font-semibold">{schema.name.raw}</h1>
        <p className="mt-1 text-body-sm text-on-surface-variant">
          Live objects discovered from the connected {snapshot.dialect} database.
        </p>
      </Surface>
      <div className="grid gap-4 lg:grid-cols-2">
        <ObjectGroup title="Tables" icon="table">
          {tables.map((table) => (
            <div className="flex items-center justify-between border-b border-outline-variant/50 px-4 py-3 last:border-0" key={table.id}>
              <span className="font-mono text-label-caps text-primary">{table.name.raw}</span>
              <span className="text-label-caps text-on-surface-variant">{table.columns.length} columns</span>
            </div>
          ))}
        </ObjectGroup>
        <ObjectGroup title="Views" icon="eye">
          {views.map((view) => (
            <div className="flex items-center justify-between border-b border-outline-variant/50 px-4 py-3 last:border-0" key={view.id}>
              <span className="font-mono text-label-caps text-primary">{view.name.raw}</span>
              <span className="text-label-caps text-on-surface-variant">{view.columns.length} columns</span>
            </div>
          ))}
        </ObjectGroup>
      </div>
      {!tables.length && !views.length ? <EmptyState compact title="No objects discovered" description="This schema has no tables or views in the current catalog snapshot." /> : null}
    </div>
  );
}

function ObjectGroup({
  title,
  icon,
  children,
}: {
  title: string;
  icon: "table" | "eye";
  children: ReactNode;
}) {
  return (
    <Surface className="overflow-hidden">
      <div className="flex items-center gap-2 border-b border-surface-divider bg-surface-subtle px-4 py-3 text-body-sm font-semibold">
        <Icon name={icon} size={14} className="text-primary" />
        {title}
      </div>
      {children}
    </Surface>
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
          <Icon name="folder" size={28} className="mb-3 text-primary" />
          <h1 className="text-body-md font-semibold">{title}</h1>
          <p className="mt-1 max-w-md text-body-sm text-on-surface-variant">{description}</p>
        </div>
      </Surface>
    </div>
  );
}
