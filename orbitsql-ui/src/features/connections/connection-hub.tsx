import { useNavigate } from "@tanstack/react-router";
import { useMemo, useState } from "react";
import { Icon } from "../../components/ui/icons";
import { toast } from "../../components/ui/toast";
import {
  Button,
  EmptyState,
  ScreenHeader,
  SegmentedTabs,
  StatusPill,
  Surface,
} from "../../components/ui/ui";
import { useCatalogStore } from "../../stores/catalog.store";
import {
  isConnectionReady,
  useConnectionsStore,
} from "../../stores/connections.store";
import { childrenFromSnapshot } from "./catalog-explorer";
import {
  connectConnection,
  connectionErrorMessage,
  disconnectConnection,
  isConnectionReconnectFailure,
  refreshConnectionCatalog,
} from "./connection-ipc";
import {
  DatabaseBrandIcon,
  databaseBrandForEngine,
} from "./database-brand-icon";
import { ProfileColorIndicator } from "./profile-color-indicator";
import { connectionProfileName } from "./profile-style";

type ConnectionFilter = "all" | "production" | "staging" | "local";

export function ConnectionHubScreen() {
  const navigate = useNavigate();
  const [filter, setFilter] = useState<ConnectionFilter>("all");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const connections = useConnectionsStore((state) => state.connections);
  const setConnectionStatus = useConnectionsStore(
    (state) => state.setConnectionStatus,
  );
  const setActiveConnection = useConnectionsStore(
    (state) => state.setActiveConnection,
  );
  const setConnectionChildren = useConnectionsStore(
    (state) => state.setConnectionChildren,
  );
  const setConnectionRefreshed = useConnectionsStore(
    (state) => state.setConnectionRefreshed,
  );
  const setCatalogSnapshot = useCatalogStore((state) => state.setSnapshot);
  const connectionList = useMemo(
    () => Object.values(connections),
    [connections],
  );
  const counts = useMemo(
    () => ({
      all: connectionList.length,
      production: connectionList.filter(
        (connection) =>
          connectionProfileName(connection.detail)?.toLowerCase() ===
          "production",
      ).length,
      staging: connectionList.filter(
        (connection) =>
          connectionProfileName(connection.detail)?.toLowerCase() === "staging",
      ).length,
      local: connectionList.filter(
        (connection) =>
          connectionProfileName(connection.detail)?.toLowerCase() === "local",
      ).length,
    }),
    [connectionList],
  );
  const visibleConnections = connectionList.filter((connection) => {
    if (filter === "all") return true;
    return connectionProfileName(connection.detail)?.toLowerCase() === filter;
  });

  async function toggleConnection(id: string) {
    setBusyId(id);
    setNotice(null);
    try {
      const connection = connections[id];
      if (!connection) return;
      if (isConnectionReady(connection.status)) {
        await disconnectConnection(id);
        setConnectionStatus(id, "disconnected");
      } else {
        setConnectionStatus(id, "connecting");
        const session = await connectConnection(id);
        setConnectionStatus(id, "introspecting");
        let snapshot = session.catalog;
        let children = childrenFromSnapshot(snapshot);
        if (children.length === 0) {
          snapshot = await refreshConnectionCatalog(id);
          children = childrenFromSnapshot(snapshot);
        }
        setCatalogSnapshot(snapshot);
        setConnectionChildren(id, children);
        setConnectionRefreshed(id);
        setConnectionStatus(id, "ready");
        setActiveConnection(id);
      }
    } catch (error) {
      setConnectionStatus(
        id,
        isConnectionReconnectFailure(error) ? "disconnected" : "error",
      );
      const message = connectionErrorMessage(error);
      setNotice(message);
      toast.add({
        description: message,
        title: "Connection failed",
        type: "error",
      });
    } finally {
      setBusyId(null);
    }
  }

  return (
    <div className="mx-auto flex max-w-[1440px] flex-col gap-5 p-5 lg:p-7">
      <ScreenHeader
        eyebrow={`${connectionList.length} saved connection${connectionList.length === 1 ? "" : "s"}`}
        title="Connections"
        description="Manage saved database connections and their live sessions."
        actions={
          <Button
            icon="plus"
            variant="primary"
            onClick={() => navigate({ to: "/connections/new" })}
          >
            New Connection
          </Button>
        }
      />
      <SegmentedTabs
        options={[
          { id: "all", label: "All", count: counts.all },
          { id: "production", label: "Production", count: counts.production },
          { id: "staging", label: "Staging", count: counts.staging },
          { id: "local", label: "Local", count: counts.local },
        ]}
        value={filter}
        onChange={(value) => setFilter(value as ConnectionFilter)}
      />
      {notice ? (
        <p className="rounded-md border border-error/30 bg-error/10 px-3 py-2 text-label-caps text-error">
          {notice}
        </p>
      ) : null}

      {visibleConnections.length > 0 ? (
        <div className="grid gap-3 xl:grid-cols-2">
          {visibleConnections.map((connection) => {
            const schemaCount = connection.children.filter(
              (child) => child.kind === "schema",
            ).length;
            const isBusy = busyId === connection.id;
            const profile =
              connectionProfileName(connection.detail) ?? "Unknown";
            const databaseBrand = databaseBrandForEngine(connection.engine);
            return (
              <Surface className="p-4" key={connection.id}>
                <div className="flex items-start justify-between gap-3">
                  <div className="flex min-w-0 items-start gap-3">
                    <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
                      {databaseBrand ? (
                        <DatabaseBrandIcon brand={databaseBrand} size={18} />
                      ) : (
                        <Icon name="database" size={18} />
                      )}
                    </span>
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <h2 className="truncate text-body-md font-semibold text-on-surface">
                          {connection.name}
                        </h2>
                        <StatusPill
                          tone={
                            isConnectionReady(connection.status)
                              ? "success"
                              : "neutral"
                          }
                        >
                          {isConnectionReady(connection.status)
                            ? "Connected"
                            : "Disconnected"}
                        </StatusPill>
                      </div>
                      <p className="mt-1 truncate font-mono text-label-caps text-on-surface-variant">
                        {connection.host}
                      </p>
                    </div>
                  </div>
                  <span className="shrink-0 font-mono text-label-caps text-on-surface-variant">
                    {connection.engine}
                  </span>
                </div>
                <div className="mt-4 grid grid-cols-2 gap-2 border-y border-outline-variant/60 py-3 text-label-caps">
                  <div>
                    <span className="block text-on-surface-variant">
                      PROFILE
                    </span>
                    <span className="mt-1 inline-flex h-6 items-center gap-1.5 px-1 font-mono text-label-caps text-on-surface">
                      <ProfileColorIndicator
                        color={connection.profileColor}
                        profile={profile}
                      />
                      {profile}
                    </span>
                  </div>
                  <div>
                    <span className="block text-on-surface-variant">
                      SCHEMAS
                    </span>
                    <span className="font-mono text-on-surface">
                      {schemaCount || "Not refreshed"}
                    </span>
                  </div>
                </div>
                <div className="flex items-center justify-between gap-3">
                  <span className="text-label-caps text-on-surface-variant">
                    {connection.lastRefreshedAt
                      ? `Refreshed ${new Date(connection.lastRefreshedAt).toLocaleString()}`
                      : "Metadata not refreshed"}
                  </span>
                  <Button
                    disabled={isBusy}
                    icon={
                      isConnectionReady(connection.status) ? "close" : "link"
                    }
                    onClick={() => void toggleConnection(connection.id)}
                    variant={
                      isConnectionReady(connection.status)
                        ? "secondary"
                        : "primary"
                    }
                  >
                    {isBusy
                      ? "Working..."
                      : isConnectionReady(connection.status)
                        ? "Disconnect"
                        : "Connect"}
                  </Button>
                </div>
              </Surface>
            );
          })}
        </div>
      ) : (
        <EmptyState
          detail="Create a PostgreSQL or Microsoft SQL Server profile to begin."
          icon="database"
          title="No saved connections"
        />
      )}
    </div>
  );
}
