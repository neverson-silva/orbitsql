import { createFileRoute } from "@tanstack/react-router";
import { SessionManagerScreen } from "../features/database/session-manager";

export const Route = createFileRoute("/session")({
  component: SessionManagerScreen,
});
