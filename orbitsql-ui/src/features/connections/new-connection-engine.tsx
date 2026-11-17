import { Outlet, useNavigate, useRouterState } from "@tanstack/react-router";
import { useState } from "react";
import { Icon } from "../../components/ui/icons";
import { Input } from "../../components/ui/input";
import {
  Button,
  IconButton,
  SegmentedTabs,
  StatusPill,
} from "../../components/ui/ui";
import { QueryWorkspaceScreen } from "../query/query-workspace";
import { ConnectionHubScreen } from "./connection-hub";
import { ConnectionModal } from "./connection-modal";
import { ConnectionStepIndicator } from "./connection-step-indicator";
import { type DatabaseBrand, DatabaseBrandIcon } from "./database-brand-icon";

const engines = [
  {
    name: "PostgreSQL",
    badge: "RECOMMENDED",
    version: "v12–v16",
    description: "v12–v16 • Popular for transactional and analytical workloads",
    color: "text-primary",
    brand: "postgresql" as DatabaseBrand,
    tags: "SSL • Pooling • Wire Protocol",
    available: true,
  },
  {
    name: "Microsoft SQL Server",
    badge: "",
    version: "2019+",
    description: "TDS driver for SQL Server, Azure SQL, and managed instances",
    color: "text-primary",
    brand: "microsoft-sql-server" as DatabaseBrand,
    tags: "TDS • Encryption • SQL Auth",
    available: true,
  },
  {
    name: "MySQL",
    badge: "",
    version: "v5.7–8.0",
    description: "InnoDB, MariaDB, and Percona Server compatible driver",
    color: "text-warning",
    brand: "mysql" as DatabaseBrand,
    tags: "SSL • Replication • InnoDB",
    available: false,
  },
  {
    name: "MariaDB",
    badge: "",
    version: "10.6+",
    description:
      "MySQL-compatible relational database with enterprise features",
    color: "text-primary",
    brand: "mariadb" as DatabaseBrand,
    tags: "SQL • Replication • Galera",
    available: false,
  },
  {
    name: "SQLite",
    badge: "",
    version: "3.x",
    description: "Testing, development, and edge engines from one file",
    color: "text-on-surface-variant",
    brand: "sqlite" as DatabaseBrand,
    tags: "WAL • JSON • FTS",
    available: false,
  },
  {
    name: "Redis",
    badge: "",
    version: "7.x",
    description: "In-memory data structures, cache inspection, and pub/sub",
    color: "text-error",
    brand: "redis" as DatabaseBrand,
    tags: "RESP3 • Pub/Sub • Streams",
    available: false,
  },
  {
    name: "Oracle Database",
    badge: "",
    version: "19c+",
    description:
      "Enterprise relational database for mission-critical workloads",
    color: "text-error",
    brand: "oracle" as DatabaseBrand,
    tags: "SQL • PL/SQL • RAC",
    available: false,
  },
];

const relationalEngines = new Set([
  "PostgreSQL",
  "MySQL",
  "MariaDB",
  "SQLite",
  "Microsoft SQL Server",
  "Oracle Database",
]);

const filterOptions = [
  { id: "all", label: "All engines", count: engines.length },
  { id: "relational", label: "Relational & SQL" },
  { id: "key-value", label: "Key-value" },
];

export function NewConnectionEngineScreen() {
  const navigate = useNavigate();
  const isConfigStep = useRouterState({
    select: (state) => state.location.pathname === "/connections/new/config",
  });
  const [engine, setEngine] = useState("PostgreSQL");
  const [filter, setFilter] = useState("all");
  const [query, setQuery] = useState("");
  const normalizedQuery = query.trim().toLowerCase();
  const filters = engines.filter((item) => {
    const matchesCategory =
      filter === "all" ||
      (filter === "relational" && relationalEngines.has(item.name)) ||
      (filter === "key-value" && item.name === "Redis");
    const matchesQuery =
      normalizedQuery.length === 0 ||
      [item.name, item.badge, item.version, item.description, item.tags].some(
        (value) => value.toLowerCase().includes(normalizedQuery),
      );
    return matchesCategory && matchesQuery;
  });
  const availableCount = filters.filter((item) => item.available).length;
  const unavailableCount = filters.length - availableCount;

  return (
    <ConnectionModal
      labelledBy={
        isConfigStep ? "connection-settings-title" : "new-connection-title"
      }
      onClose={() => navigate({ to: "/" })}
      size={isConfigStep ? "config" : "engine"}
      underlay={
        isConfigStep ? (
          <QueryWorkspaceScreen embedded />
        ) : (
          <ConnectionHubScreen />
        )
      }
    >
      {isConfigStep ? (
        <Outlet />
      ) : (
        <div className="flex min-h-0 h-full w-full flex-col overflow-hidden rounded-lg border border-floating-border bg-dialog-surface shadow-overlay">
          <header className="flex shrink-0 items-start justify-between gap-4 border-b border-outline-variant/70 px-5 py-4">
            <div className="min-w-0">
              <p className="font-mono text-label-caps font-semibold uppercase ds-tracking-caps-wide text-primary">
                New connection
              </p>
              <h1
                className="text-body-lg font-semibold text-on-surface"
                id="new-connection-title"
              >
                Choose a database engine
              </h1>
              <p className="mt-0.5 text-label-caps text-on-surface-variant">
                Select the engine you want to connect to.
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-3">
              <ConnectionStepIndicator activeStep={1} />
              <IconButton
                label="Close"
                name="close"
                onClick={() => navigate({ to: "/" })}
                size="md"
              />
            </div>
          </header>
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <div className="flex shrink-0 flex-wrap items-center gap-3 border-b border-outline-variant/70 px-5 py-3">
              <div className="relative min-w-52 flex-1 sm:max-w-xs">
                <Icon
                  className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-on-surface-variant"
                  name="search"
                  size={14}
                />
                <Input
                  aria-label="Search database engines"
                  className="h-9 pl-9 pr-9 text-body-sm"
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="Search engines"
                  value={query}
                />
                {query ? (
                  <button
                    aria-label="Clear engine search"
                    className="absolute top-1/2 right-2 flex h-6 w-6 -translate-y-1/2 cursor-pointer items-center justify-center rounded-md text-on-surface-variant hover:bg-control-surface-hover hover:text-on-surface"
                    onClick={() => setQuery("")}
                    type="button"
                  >
                    <Icon name="close" size={13} />
                  </button>
                ) : null}
              </div>
              <SegmentedTabs
                className="shrink-0"
                options={filterOptions}
                value={filter}
                onChange={setFilter}
              />
            </div>
            <div className="flex min-h-0 flex-1 flex-col">
              <div className="flex shrink-0 items-center justify-between gap-3 px-5 pb-1 pt-3">
                <p className="text-label-caps font-medium text-on-surface">
                  {availableCount} {availableCount === 1 ? "driver" : "drivers"}{" "}
                  available
                  {unavailableCount > 0 ? (
                    <span className="ml-2 text-on-surface-variant">
                      · {unavailableCount} coming soon
                    </span>
                  ) : null}
                </p>
                <p className="hidden text-label-caps text-on-surface-variant sm:block">
                  Choose one to continue
                </p>
              </div>
              <div
                aria-label="Database engines"
                className="grid min-h-0 flex-1 auto-rows-max gap-3 overflow-y-auto p-5 pt-2 sm:grid-cols-2"
                role="radiogroup"
              >
                {filters.map((item) => {
                  const selected = engine === item.name;
                  return (
                    <Button
                      ariaChecked={selected}
                      disabled={!item.available}
                      className={
                        selected
                          ? "h-auto min-h-16 w-full items-stretch justify-start rounded-md border border-primary bg-selected-surface p-0 text-left whitespace-normal ring-1 ring-inset ring-primary/25 transition-[background-color,border-color,box-shadow]"
                          : item.available
                            ? "h-auto min-h-16 w-full items-stretch justify-start rounded-md border border-control-border p-0 text-left whitespace-normal transition-[background-color,border-color] hover:border-control-border-hover hover:bg-control-surface-hover"
                            : "h-auto min-h-16 w-full cursor-not-allowed items-stretch justify-start rounded-md border border-control-border-disabled bg-control-surface-disabled p-0 text-left whitespace-normal opacity-60"
                      }
                      key={item.name}
                      onClick={() => setEngine(item.name)}
                      role="radio"
                      type="button"
                      variant="secondary"
                    >
                      <div className="flex min-h-16 w-full items-center gap-2.5 p-2.5">
                        <span
                          className={
                            "flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-control-border bg-control-surface-active " +
                            item.color
                          }
                        >
                          <DatabaseBrandIcon brand={item.brand} size={17} />
                        </span>
                        <div className="min-w-0 flex-1">
                          <h2 className="truncate text-body-sm font-semibold text-on-surface">
                            {item.name}
                          </h2>
                          <div className="mt-0.5 flex min-w-0 items-center gap-2">
                            <span className="font-mono text-label-caps text-on-surface-variant">
                              {item.version}
                            </span>
                            {item.badge || !item.available ? (
                              <StatusPill
                                className="h-5 shrink-0 px-1.5"
                                tone={item.available ? "success" : "neutral"}
                              >
                                {item.badge || "Coming soon"}
                              </StatusPill>
                            ) : null}
                          </div>
                        </div>
                        <span
                          className={
                            selected
                              ? "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-primary bg-primary text-on-primary"
                              : "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-outline text-transparent"
                          }
                        >
                          <Icon name="check" size={12} />
                        </span>
                      </div>
                    </Button>
                  );
                })}
                {filters.length === 0 ? (
                  <div className="col-span-full flex min-h-40 flex-col items-center justify-center rounded-md border border-dashed border-surface-divider bg-surface-subtle p-6 text-center">
                    <Icon
                      className="mb-2 text-on-surface-variant"
                      name="search"
                      size={20}
                    />
                    <p className="text-body-sm font-medium text-on-surface">
                      No engines found
                    </p>
                    <p className="mt-1 text-label-caps text-on-surface-variant">
                      Try another search or filter.
                    </p>
                  </div>
                ) : null}
              </div>
            </div>
          </div>
          <footer className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-t border-outline-variant/70 px-5 py-3">
            <span className="hidden items-center gap-2 text-label-caps text-on-surface-variant sm:flex">
              <Icon name="link" size={13} />
              Import from connection string or URI
            </span>
            <div className="ml-auto flex items-center gap-2">
              <Button variant="secondary" onClick={() => navigate({ to: "/" })}>
                Cancel
              </Button>
              <Button
                variant="primary"
                icon="chevron-right"
                onClick={() =>
                  navigate({
                    to: "/connections/new/config",
                    search: { connectionId: undefined, engine },
                  })
                }
              >
                Continue to Configuration
              </Button>
            </div>
          </footer>
        </div>
      )}
    </ConnectionModal>
  );
}
