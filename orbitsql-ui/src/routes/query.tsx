import { createFileRoute } from "@tanstack/react-router";
import { QueryWorkspaceScreen } from "../features/query/query-workspace";

export const Route = createFileRoute("/query")({
  component: QueryWorkspaceScreen,
});
