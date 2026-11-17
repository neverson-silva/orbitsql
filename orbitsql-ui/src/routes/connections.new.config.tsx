import { createFileRoute } from "@tanstack/react-router";
import { NewConnectionConfigScreen } from "../features/connections/new-connection-config";

export const Route = createFileRoute("/connections/new/config")({
  validateSearch: (search: Record<string, unknown>) => ({
    connectionId:
      typeof search.connectionId === "string" ? search.connectionId : undefined,
    engine: typeof search.engine === "string" ? search.engine : undefined,
  }),
  component: NewConnectionConfigScreen,
});
