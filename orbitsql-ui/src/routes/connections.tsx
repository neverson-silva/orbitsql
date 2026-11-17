import { createFileRoute, Outlet } from "@tanstack/react-router";
import { ConnectionHubScreen } from "../features/connections/connection-hub";

export const Route = createFileRoute("/connections")({
  component: ConnectionsRouteScreen,
});

function ConnectionsRouteScreen() {
  return (
    <>
      <ConnectionHubScreen />
      <Outlet />
    </>
  );
}
