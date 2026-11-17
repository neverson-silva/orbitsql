import { createFileRoute } from "@tanstack/react-router";
import { NewConnectionEngineScreen } from "../features/connections/new-connection-engine";

export const Route = createFileRoute("/connections/new")({
  component: NewConnectionEngineScreen,
});
