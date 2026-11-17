import { createFileRoute } from "@tanstack/react-router";
import { ConnectionHubScreen } from "../features/connections/connection-hub";

export const Route = createFileRoute("/")({
  component: ConnectionHubScreen,
});
