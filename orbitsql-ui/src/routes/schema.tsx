import { createFileRoute } from "@tanstack/react-router";
import { SchemaInspectorScreen } from "../features/database/schema-inspector";

export const Route = createFileRoute("/schema")({
  validateSearch: (search: Record<string, unknown>) => ({
    connectionId:
      typeof search.connectionId === "string" ? search.connectionId : undefined,
    schemaId: typeof search.schemaId === "string" ? search.schemaId : undefined,
  }),
  component: SchemaInspectorScreen,
});
