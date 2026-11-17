import { Button } from "../../components/ui/button";
import { Icon } from "../../components/ui/icons";
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "../../components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../../components/ui/select";
import type { ExplorerConnection } from "../../stores/connections.store";
import {
  type ContextDimensionDescriptor,
  type ContextDimensionId,
  dialectForConnection,
  type ExecutionContext,
  formatExecutionContext,
  getExecutionContextCapabilities,
} from "../connections/execution-context";
import type { CatalogIndex } from "../sql-intelligence/catalog";

const allSchemasOption = "__orbit_all_schemas__";
const clearSchemaOption = "__orbit_clear_schema__";

function statusTone(status: ExplorerConnection["status"]) {
  if (status === "connecting" || status === "introspecting")
    return "bg-warning";
  if (status === "error") return "bg-error";
  if (status === "connected" || status === "ready") return "bg-success";
  return "bg-on-surface-variant";
}

function optionsFor(
  dimension: ContextDimensionId,
  catalog: CatalogIndex | undefined,
  context: ExecutionContext,
) {
  if (dimension === "database") {
    return (
      catalog?.snapshot.databases.map((item) => item.name.raw) ??
      (context.dimensions.database ? [context.dimensions.database] : [])
    );
  }
  if (dimension === "schema") {
    const schemas =
      catalog?.snapshot.schemas.map((item) => item.name.raw) ??
      (context.dimensions.schema ? [context.dimensions.schema] : []);
    return [allSchemasOption, clearSchemaOption, ...schemas];
  }
  return context.dimensions[dimension] ? [context.dimensions[dimension]] : [];
}

function DimensionSelect({
  descriptor,
  catalog,
  context,
  onSelect,
}: {
  descriptor: ContextDimensionDescriptor;
  catalog: CatalogIndex | undefined;
  context: ExecutionContext;
  onSelect: (dimension: ContextDimensionId, value: string) => void;
}) {
  const options = optionsFor(descriptor.id, catalog, context);
  const value = context.dimensions[descriptor.id] ?? "";
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between gap-2 text-label-caps text-on-surface-variant">
        <span>{descriptor.label}</span>
        {descriptor.selectionMode === "reconnect" ? (
          <span className="text-[9px] text-on-surface-variant">reconnect</span>
        ) : null}
      </div>
      <Select
        onValueChange={(next) => {
          if (!next) return;
          if (descriptor.id === "schema" && next === allSchemasOption) {
            onSelect(descriptor.id, "");
            return;
          }
          if (descriptor.id === "schema" && next === clearSchemaOption) {
            onSelect(descriptor.id, "");
            return;
          }
          onSelect(descriptor.id, next);
        }}
        value={value}
      >
        <SelectTrigger
          aria-label={descriptor.label}
          className="h-8 w-full"
          disabled={
            descriptor.selectionMode === "fixed" || options.length === 0
          }
          size="sm"
        >
          <SelectValue
            placeholder={
              descriptor.id === "schema"
                ? "All schemas"
                : `Select ${descriptor.label.toLowerCase()}`
            }
          />
        </SelectTrigger>
        <SelectContent alignItemWithTrigger>
          {options.map((option) => {
            const label =
              option === allSchemasOption
                ? "All schemas"
                : option === clearSchemaOption
                  ? "Clear selection"
                  : option;
            return (
              <SelectItem key={option} value={option}>
                {label}
              </SelectItem>
            );
          })}
        </SelectContent>
      </Select>
    </div>
  );
}

export function ExecutionContextPicker({
  connection,
  connections,
  catalog,
  context,
  onConnectionSelect,
  onSelect,
}: {
  connection: ExplorerConnection | undefined;
  connections: readonly ExplorerConnection[];
  catalog: CatalogIndex | undefined;
  context: ExecutionContext;
  onConnectionSelect: (connectionId: string) => void;
  onSelect: (dimension: ContextDimensionId, value: string) => void;
}) {
  const capabilities = getExecutionContextCapabilities(
    dialectForConnection(connection),
  );
  const label = formatExecutionContext(context, capabilities);
  const engine = connection?.engine ?? "Database";
  const stateLabel = connection?.status ?? "disconnected";
  const targetLabel = connection
    ? label === "No context selected"
      ? engine
      : `${label} · ${engine}`
    : "Select execution context";
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            aria-label={`Execution context: ${connection?.name ?? "No connection"}, ${targetLabel}, ${stateLabel}`}
            className="h-9 w-full min-w-0 max-w-full justify-start gap-2 px-2"
            variant="outline"
          />
        }
      >
        <span
          aria-hidden="true"
          className={`size-1.5 shrink-0 rounded-full ${statusTone(connection?.status ?? "disconnected")}`}
        />
        <span className="flex min-w-0 flex-1 items-center overflow-hidden">
          <span className="w-full truncate text-label-md font-medium text-on-surface">
            {connection?.name ?? "No connection"}
          </span>
        </span>
        <Icon
          aria-hidden="true"
          className="ml-auto shrink-0 text-on-surface-variant"
          name="chevron-down"
          size={12}
        />
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="max-h-[calc(100vh-1rem)] w-80 gap-3 overflow-y-auto"
        side="right"
        sideOffset={8}
      >
        <PopoverHeader>
          <PopoverTitle className="flex items-center gap-2">
            <span
              aria-hidden="true"
              className={`size-1.5 rounded-full ${statusTone(connection?.status ?? "disconnected")}`}
            />
            Switch execution context
          </PopoverTitle>
          <PopoverDescription>
            {connection?.name ?? "No connection"} · {stateLabel}
          </PopoverDescription>
        </PopoverHeader>
        <div className="space-y-2 border-t border-outline-variant/60 pt-2">
          <p className="font-mono text-label-caps text-primary">
            Execution context
          </p>
          <div className="space-y-1">
            <span className="text-label-caps text-on-surface-variant">
              Connection
            </span>
            <Select
              onValueChange={(next) => {
                if (next) onConnectionSelect(next);
              }}
              value={context.connectionId ?? ""}
            >
              <SelectTrigger
                aria-label="Connection"
                className="h-8 w-full"
                size="sm"
              >
                <SelectValue placeholder="Select connection">
                  {connections.find((item) => item.id === context.connectionId)
                    ?.name ?? "Select connection"}
                </SelectValue>
              </SelectTrigger>
              <SelectContent alignItemWithTrigger>
                {connections.map((item) => (
                  <SelectItem key={item.id} value={item.id}>
                    {item.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          {capabilities.map((descriptor) => (
            <DimensionSelect
              catalog={catalog}
              context={context}
              descriptor={descriptor}
              key={descriptor.id}
              onSelect={onSelect}
            />
          ))}
        </div>
        <div className="border-t border-outline-variant/60 pt-2 text-label-caps text-on-surface-variant">
          {stateLabel}
        </div>
      </PopoverContent>
    </Popover>
  );
}
