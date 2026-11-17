import { useNavigate, useSearch } from "@tanstack/react-router";
import { type ComponentProps, useEffect, useState } from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "../../components/ui/alert-dialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Icon } from "../../components/ui/icons";
import { Input } from "../../components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "../../components/ui/input-group";
import {
  PropertyRow,
  PropertySection,
} from "../../components/ui/property-grid";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../../components/ui/select";
import { Tabs, TabsList, TabsTrigger } from "../../components/ui/tabs";
import { toast } from "../../components/ui/toast";
import { Button, IconButton, StatusPill } from "../../components/ui/ui";
import { useCatalogStore } from "../../stores/catalog.store";
import {
  isConnectionReady,
  useConnectionsStore,
} from "../../stores/connections.store";
import { childrenFromSnapshot } from "./catalog-explorer";
import {
  type ConnectionAuthMethod,
  type ConnectionDiagnostic,
  type ConnectionProfileDefinition,
  type ConnectionTransport,
  connectConnection,
  connectionErrorMessage,
  deleteConnectionProfileDefinition,
  listConnectionProfileDefinitions,
  listConnectionStatuses,
  listSavedConnections,
  refreshConnectionCatalog,
  saveConnectionProfile,
  testConnection,
  updateConnection,
  upsertConnectionProfileDefinition,
} from "./connection-ipc";
import { ConnectionStepIndicator } from "./connection-step-indicator";
import { profileColorHex } from "./profile-style";

const tabs = ["General", "Driver Properties"];

type AuthenticationMethod = "password" | "windowsNtlm";

type AuthenticationOption = {
  value: AuthenticationMethod;
  label: string;
};

function authenticationOptionsFor(engine: string): AuthenticationOption[] {
  if (engine === "Microsoft SQL Server") {
    return [
      { value: "password", label: "SQL Authentication" },
      { value: "windowsNtlm", label: "Windows Authentication (NTLM)" },
    ];
  }

  return [{ value: "password", label: "Password" }];
}

const profileColors = [
  { label: "Lime", value: "#84cc16" },
  { label: "Amber", value: "#f59e0b" },
  { label: "Red", value: "#ef4444" },
  { label: "Blue", value: "#38bdf8" },
  { label: "Violet", value: "#a78bfa" },
  { label: "Pink", value: "#f472b6" },
] as const;

type DriverPropertyDefinition = {
  key: string;
  label: string;
  defaultValue: string;
  kind: "boolean" | "select" | "text";
  description: string;
  options?: ReadonlyArray<{ label: string; value: string }>;
};

const defaultTransport: ConnectionTransport = {
  mode: "direct",
  sshHost: "",
  sshPort: 22,
  sshUserName: "",
  sshPrivateKeyPath: "",
  proxyHost: "",
  proxyPort: 1080,
  proxyType: "socks5",
};

const defaultDriverProperties: Record<string, string> = {
  applicationName: "Orbit SQL",
  connectTimeoutSeconds: "15",
  statementTimeoutMilliseconds: "0",
  statementCacheCapacity: "100",
  sslMode: "prefer",
  extraFloatDigits: "3",
  searchPath: "",
};

const defaultSqlServerDriverProperties: Record<string, string> = {
  applicationName: "Orbit SQL",
  connectTimeoutSeconds: "15",
  requestTimeoutSeconds: "15",
  packetSize: "4096",
  multiSubnetFailover: "false",
  encrypt: "true",
  fallbackToDefaultDb: "false",
  useUtc: "true",
  rowCollectionOnDone: "false",
};

function driverCatalogLabel(engine: string) {
  return engine === "Microsoft SQL Server"
    ? "Show all databases"
    : "Show all schemas";
}

function ProfileColorDot({
  color,
  className,
}: {
  color: string;
  className?: string;
}) {
  return (
    <span
      aria-hidden="true"
      className={`size-2 shrink-0 rounded-full ring-2 ring-surface-floating ${className ?? ""}`}
      style={{ backgroundColor: profileColorHex(color) }}
    />
  );
}

function connectionProfileName(
  profileId: string,
  profiles: ConnectionProfileDefinition[],
): string {
  const profile =
    profiles.find((item) => item.id === profileId) ??
    profiles.find(
      (item) => item.name.toLowerCase() === profileId.toLowerCase(),
    );
  return profile?.name ?? profileId;
}

function connectionDetail(
  profileId: string,
  database: string,
  profiles: ConnectionProfileDefinition[],
): string {
  const name = connectionProfileName(profileId, profiles);
  return database ? `${name} • ${database}` : name;
}

function engineFor(engine: string): "postgresql" | "sqlserver" {
  return engine === "Microsoft SQL Server" ? "sqlserver" : "postgresql";
}

function defaultDriverPropertiesFor(engine: string): Record<string, string> {
  return engineFor(engine) === "sqlserver"
    ? { ...defaultSqlServerDriverProperties }
    : { ...defaultDriverProperties };
}

export function NewConnectionConfigScreen() {
  const navigate = useNavigate();
  const search = useSearch({ from: "/connections/new/config" }) as {
    connectionId?: string;
    engine?: string;
  };
  const engine = search.engine ?? "PostgreSQL";
  const editingConnectionId = search.connectionId;
  const isEditing = Boolean(editingConnectionId);
  const [activeTab, setActiveTab] = useState("General");
  const [profiles, setProfiles] = useState<ConnectionProfileDefinition[]>([]);
  const [activeProfileId, setActiveProfileId] = useState("");
  const [editingConnectionProfileId, setEditingConnectionProfileId] =
    useState("");
  const [connectionName, setConnectionName] = useState("");
  const [server, setServer] = useState("");
  const [port, setPort] = useState(
    engineFor(engine) === "sqlserver" ? "1433" : "5432",
  );
  const [userName, setUserName] = useState("");
  const [password, setPassword] = useState("");
  const [database, setDatabase] = useState("");
  const [trustServerCertificate, setTrustServerCertificate] = useState(false);
  const [showAllCatalogItems, setShowAllCatalogItems] = useState(true);
  const [keepAliveEnabled, setKeepAliveEnabled] = useState(true);
  const [keepAliveIntervalSeconds, setKeepAliveIntervalSeconds] =
    useState("15");
  const [idleTimeoutSeconds, setIdleTimeoutSeconds] = useState("");
  const [transport, setTransport] =
    useState<ConnectionTransport>(defaultTransport);
  const [driverProperties, setDriverProperties] = useState(() =>
    defaultDriverPropertiesFor(engine),
  );
  const [authMethod, setAuthMethod] =
    useState<ConnectionAuthMethod>("password");
  const [busy, setBusy] = useState(false);
  const [reconnectPromptOpen, setReconnectPromptOpen] = useState(false);
  const [operation, setOperation] = useState<"idle" | "testing" | "saving">(
    "idle",
  );
  const [diagnostic, setDiagnostic] = useState<ConnectionDiagnostic | null>(
    null,
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const addConnection = useConnectionsStore((state) => state.addConnection);
  const setActiveConnection = useConnectionsStore(
    (state) => state.setActiveConnection,
  );
  const setConnectionStatus = useConnectionsStore(
    (state) => state.setConnectionStatus,
  );
  const activeProfile = profiles.find(
    (profile) => profile.id === activeProfileId,
  );

  useEffect(() => {
    let mounted = true;
    listConnectionProfileDefinitions()
      .then((loadedProfiles) => {
        if (!mounted) return;
        setProfiles(loadedProfiles);
        setActiveProfileId((currentId) =>
          loadedProfiles.some((profile) => profile.id === currentId)
            ? currentId
            : (loadedProfiles[0]?.id ?? ""),
        );
      })
      .catch((error) => {
        if (mounted) setErrorMessage(connectionErrorMessage(error));
      });
    return () => {
      mounted = false;
    };
  }, []);

  useEffect(() => {
    if (!editingConnectionId) return;
    let mounted = true;
    listSavedConnections()
      .then((profiles) => {
        if (!mounted) return;
        const profile = profiles.find(
          (candidate) => candidate.id === editingConnectionId,
        );
        if (!profile) {
          setErrorMessage("Connection profile was not found.");
          return;
        }
        setConnectionName(profile.name);
        setServer(profile.server);
        setPort(String(profile.port));
        setUserName(profile.userName);
        setDatabase(profile.database);
        setTrustServerCertificate(profile.trustServerCertificate);
        setAuthMethod(profile.authMethod);
        setShowAllCatalogItems(profile.showAllCatalogItems);
        setEditingConnectionProfileId(profile.profileId);
        setKeepAliveEnabled(profile.keepAliveEnabled);
        setKeepAliveIntervalSeconds(String(profile.keepAliveIntervalSeconds));
        setIdleTimeoutSeconds(
          profile.idleTimeoutSeconds === null
            ? ""
            : String(profile.idleTimeoutSeconds),
        );
      })
      .catch((error) => {
        if (mounted) setErrorMessage(connectionErrorMessage(error));
      });
    return () => {
      mounted = false;
    };
  }, [editingConnectionId]);

  useEffect(() => {
    if (engineFor(engine) === "postgresql") setAuthMethod("password");
  }, [engine]);

  useEffect(() => {
    if (!isEditing || !editingConnectionProfileId) return;
    const matchingProfile = profiles.find(
      (profile) =>
        profile.id === editingConnectionProfileId ||
        profile.name.toLowerCase() === editingConnectionProfileId.toLowerCase(),
    );
    if (matchingProfile && matchingProfile.id !== activeProfileId) {
      setActiveProfileId(matchingProfile.id);
    }
  }, [activeProfileId, editingConnectionProfileId, isEditing, profiles]);

  const draft = {
    name: connectionName,
    dialect: engineFor(engine),
    server,
    port: Number(port),
    userName,
    password,
    database,
    trustServerCertificate,
    authMethod,
    profileId: activeProfile?.id ?? editingConnectionProfileId,
    showAllCatalogItems,
    transport,
    driverProperties,
    keepAliveEnabled,
    keepAliveIntervalSeconds: Number(keepAliveIntervalSeconds),
    idleTimeoutSeconds: idleTimeoutSeconds ? Number(idleTimeoutSeconds) : null,
  } as const;

  async function runTest() {
    setBusy(true);
    setOperation("testing");
    setErrorMessage(null);
    setDiagnostic(null);
    try {
      setDiagnostic(await testConnection(draft, editingConnectionId));
    } catch (error) {
      setDiagnostic(null);
      setErrorMessage(connectionErrorMessage(error));
    } finally {
      setBusy(false);
      setOperation("idle");
    }
  }

  const testStatus = busy
    ? operation === "saving"
      ? {
          detail: "Saving, connecting and loading catalog…",
          icon: "refresh" as const,
          label: "Connecting",
          tone: "warning" as const,
        }
      : {
          detail: "Checking host and credentials…",
          icon: "refresh" as const,
          label: "Testing",
          tone: "warning" as const,
        }
    : diagnostic?.success
      ? {
          detail: `Host reachable · ${diagnostic.latencyMs}ms`,
          icon: "check" as const,
          label: "Connected",
          tone: "success" as const,
        }
      : diagnostic
        ? {
            detail:
              diagnostic.errorMessage ??
              diagnostic.errorCode ??
              "The connection test failed.",
            icon: "warning" as const,
            label: "Failed",
            tone: "error" as const,
          }
        : errorMessage
          ? {
              detail: errorMessage,
              icon: "warning" as const,
              label: "Failed",
              tone: "error" as const,
            }
          : {
              detail: "Run a test to verify this connection.",
              icon: "circle" as const,
              label: "Not tested",
              tone: "neutral" as const,
            };

  async function saveAndConnect(reconnect: boolean) {
    setBusy(true);
    setOperation("saving");
    setErrorMessage(null);
    setDiagnostic(null);
    try {
      if (isEditing && editingConnectionId) {
        const updated = await updateConnection(
          {
            id: editingConnectionId,
            name: connectionName,
            dialect: engineFor(engine),
            server,
            port: Number(port),
            userName,
            password: password.trim() ? password : null,
            database,
            trustServerCertificate,
            authMethod,
            profileId: activeProfile?.id ?? editingConnectionProfileId,
            showAllCatalogItems,
            keepAliveEnabled,
            keepAliveIntervalSeconds: Number(keepAliveIntervalSeconds),
            idleTimeoutSeconds: idleTimeoutSeconds
              ? Number(idleTimeoutSeconds)
              : null,
          },
          reconnect,
        );
        const store = useConnectionsStore.getState();
        store.updateConnection(editingConnectionId, {
          name: updated.name,
          host: updated.server,
          profileId: updated.profileId,
          profileColor: activeProfile?.color,
          detail: connectionDetail(
            updated.profileId,
            updated.database,
            profiles,
          ),
        });
        if (
          reconnect &&
          (await listConnectionStatuses()).includes(editingConnectionId)
        ) {
          const snapshot = await refreshConnectionCatalog(editingConnectionId);
          useCatalogStore.getState().setSnapshot(snapshot);
          store.setConnectionChildren(
            editingConnectionId,
            childrenFromSnapshot(snapshot),
          );
          store.setConnectionRefreshed(editingConnectionId);
        }
        toast.add({
          description: "Connection settings updated.",
          title: "Connection updated",
          type: "success",
        });
        navigate({ to: "/query" });
        return;
      }
      const profile = await saveConnectionProfile(draft);
      addConnection({
        id: profile.id,
        name: profile.name,
        engine,
        profileId: profile.profileId,
        profileColor: activeProfile?.color,
        detail: connectionDetail(profile.profileId, profile.database, profiles),
        host: profile.server,
        status: "connecting",
        expanded: true,
        children: [],
      });
      setActiveConnection(profile.id);
      const session = await connectConnection(profile.id);
      setConnectionStatus(profile.id, "introspecting");
      navigate({ to: "/connections" });
      let snapshot = session.catalog;
      let children = childrenFromSnapshot(snapshot);
      if (children.length === 0) {
        snapshot = await refreshConnectionCatalog(profile.id);
        children = childrenFromSnapshot(snapshot);
      }
      useCatalogStore.getState().setSnapshot(snapshot);
      const store = useConnectionsStore.getState();
      store.setConnectionChildren(profile.id, children);
      store.setConnectionRefreshed(profile.id);
      store.setConnectionStatus(profile.id, "ready");
    } catch (error) {
      setErrorMessage(connectionErrorMessage(error));
    } finally {
      setBusy(false);
      setOperation("idle");
    }
  }

  async function requestSaveChanges() {
    if (!isEditing || !editingConnectionId) {
      void saveAndConnect(false);
      return;
    }
    setBusy(true);
    let connected = false;
    try {
      connected = (await listConnectionStatuses()).includes(
        editingConnectionId,
      );
    } catch {
      connected = isConnectionReady(
        useConnectionsStore.getState().connections[editingConnectionId]
          ?.status ?? "disconnected",
      );
    }
    setBusy(false);
    if (connected) {
      setReconnectPromptOpen(true);
      return;
    }
    void saveAndConnect(false);
  }

  return (
    <div className="flex min-h-0 h-full w-full min-w-0 flex-col overflow-hidden rounded-lg border border-floating-border bg-dialog-surface shadow-overlay">
      <h1 className="sr-only" id="connection-settings-title">
        Connection Settings
      </h1>
      <div className="flex shrink-0 items-center justify-between border-b border-outline-variant/70 px-5 py-1.5">
        <div className="flex min-w-0 items-center gap-2">
          <IconButton
            label="Back to engine"
            name="chevron-left"
            onClick={() =>
              navigate({ to: isEditing ? "/query" : "/connections/new" })
            }
          />
          <span className="truncate text-body-sm font-medium text-on-surface">
            {isEditing ? `Edit ${engine} connection` : `${engine} connection`}
          </span>
        </div>
        <div className="flex items-center gap-3">
          <ConnectionStepIndicator activeStep={2} />
          <IconButton
            label="Close"
            name="close"
            onClick={() => navigate({ to: isEditing ? "/query" : "/" })}
          />
        </div>
      </div>
      <div className="shrink-0 border-b border-outline-variant/70 px-5 py-1">
        <div className="flex flex-wrap items-center gap-1">
          <Tabs
            value={activeTab}
            onValueChange={(value) => {
              if (value) setActiveTab(value);
            }}
          >
            <TabsList className="h-7 gap-2 p-0" variant="line">
              {tabs.map((tab) => (
                <TabsTrigger
                  className="h-7 px-1 text-label-caps"
                  key={tab}
                  value={tab}
                >
                  {tab}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
          <span className="ml-auto hidden font-mono text-label-caps text-on-surface-variant sm:block">
            v42.7.2 JDBC Compatible
          </span>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {activeTab === "General" ? (
          <div className="space-y-5 p-4 sm:p-5">
            <PropertySection title="Connection">
              <div className="grid gap-2 md:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <PropertyRow label="Name" htmlFor="connection-name">
                  <ConnectionInput
                    id="connection-name"
                    value={connectionName}
                    onChange={setConnectionName}
                  />
                </PropertyRow>
                <PropertyRow label="Profile" htmlFor="active-profile-select">
                  <ProfilePanel
                    activeProfileId={activeProfileId}
                    onActiveProfileChange={setActiveProfileId}
                    onProfilesChange={setProfiles}
                    profiles={profiles}
                  />
                </PropertyRow>
              </div>
              <div className="grid gap-2 md:grid-cols-[minmax(0,1fr)_minmax(8rem,0.35fr)]">
                <PropertyRow label="Host" htmlFor="connection-host">
                  <ConnectionInput
                    id="connection-host"
                    prefix="dns"
                    value={server}
                    onChange={setServer}
                  />
                </PropertyRow>
                <PropertyRow label="Port" htmlFor="connection-port">
                  <ConnectionInput
                    id="connection-port"
                    value={port}
                    onChange={setPort}
                  />
                </PropertyRow>
              </div>
              <PropertyRow
                label="Database (optional)"
                htmlFor="connection-database"
              >
                <ConnectionInput
                  id="connection-database"
                  prefix="database"
                  placeholder="Use server default"
                  value={database}
                  onChange={setDatabase}
                />
              </PropertyRow>
              <div className="grid gap-2 md:grid-cols-2">
                <PropertyRow label="Catalog scope" htmlFor="catalog-scope">
                  <label
                    className="flex min-h-8 cursor-pointer items-center gap-2 text-body-sm text-on-surface"
                    htmlFor="catalog-scope"
                  >
                    <Input
                      checked={showAllCatalogItems}
                      className="size-4 accent-primary"
                      id="catalog-scope"
                      onChange={(event) =>
                        setShowAllCatalogItems(event.target.checked)
                      }
                      type="checkbox"
                    />
                    {driverCatalogLabel(engine)}
                  </label>
                </PropertyRow>
                <PropertyRow
                  label="Trust certificate"
                  htmlFor="trust-server-certificate"
                >
                  <label
                    className="flex min-h-8 cursor-pointer items-center gap-2 text-body-sm text-on-surface"
                    htmlFor="trust-server-certificate"
                  >
                    <Input
                      checked={trustServerCertificate}
                      className="size-4 accent-primary"
                      id="trust-server-certificate"
                      onChange={(event) =>
                        setTrustServerCertificate(event.target.checked)
                      }
                      type="checkbox"
                    />
                    Trust server certificate
                  </label>
                </PropertyRow>
              </div>
            </PropertySection>
            <PropertySection title="Connection lifecycle">
              <div className="grid gap-2 md:grid-cols-2">
                <PropertyRow
                  label="Keep connection alive"
                  htmlFor="keep-alive-enabled"
                >
                  <label
                    className="flex min-h-8 cursor-pointer items-center gap-2 text-body-sm text-on-surface"
                    htmlFor="keep-alive-enabled"
                  >
                    <Input
                      checked={keepAliveEnabled}
                      className="size-4 accent-primary"
                      id="keep-alive-enabled"
                      onChange={(event) =>
                        setKeepAliveEnabled(event.target.checked)
                      }
                      type="checkbox"
                    />
                    Send periodic health checks
                  </label>
                </PropertyRow>
                <PropertyRow
                  label="Keep-alive interval (seconds)"
                  htmlFor="keep-alive-interval"
                >
                  <ConnectionInput
                    id="keep-alive-interval"
                    onChange={setKeepAliveIntervalSeconds}
                    value={keepAliveIntervalSeconds}
                  />
                </PropertyRow>
              </div>
              <PropertyRow
                label="Close idle connection after (seconds)"
                htmlFor="idle-timeout"
              >
                <ConnectionInput
                  id="idle-timeout"
                  onChange={setIdleTimeoutSeconds}
                  value={idleTimeoutSeconds}
                />
              </PropertyRow>
            </PropertySection>
            <AuthenticationPanel
              engine={engine}
              method={authMethod}
              passwordPlaceholder={
                isEditing ? "Saved securely · leave blank to keep" : undefined
              }
              onMethodChange={setAuthMethod}
              password={password}
              userName={userName}
              onPasswordChange={setPassword}
              onUserNameChange={setUserName}
            />
            <PropertySection title="Connection mode">
              <div>
                <PropertyRow label="Transport" htmlFor="connection-transport">
                  <Select
                    value={transport.mode}
                    onValueChange={(value) => {
                      if (
                        value === "direct" ||
                        value === "ssh" ||
                        value === "proxy"
                      ) {
                        setTransport((current) => ({
                          ...current,
                          mode: value,
                        }));
                      }
                    }}
                  >
                    <SelectTrigger
                      className="h-8 w-full text-body-sm"
                      id="connection-transport"
                    >
                      <SelectValue>
                        {transport.mode === "direct"
                          ? "Direct TCP/IP"
                          : transport.mode === "ssh"
                            ? "SSH tunnel"
                            : "SOCKS5 / HTTP proxy"}
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent align="start">
                      <SelectItem value="direct">Direct TCP/IP</SelectItem>
                      <SelectItem value="ssh">SSH tunnel</SelectItem>
                      <SelectItem value="proxy">SOCKS5 / HTTP proxy</SelectItem>
                    </SelectContent>
                  </Select>
                </PropertyRow>
              </div>
              <TransportFields transport={transport} onChange={setTransport} />
            </PropertySection>
            <div className="flex min-h-8 items-center gap-2 border-t border-surface-divider pt-2 text-label-caps text-on-surface-variant">
              <Icon name="bolt" size={13} className="shrink-0 text-primary" />
              <span>Binary protocol · Pool 16</span>
              <Button className="ml-auto h-7 px-2 text-primary" variant="ghost">
                Tune Session Limits
              </Button>
            </div>
          </div>
        ) : (
          <div className="p-4 sm:p-5">
            <DriverPropertiesPanel
              engine={engine}
              properties={driverProperties}
              onPropertiesChange={setDriverProperties}
            />
          </div>
        )}
      </div>
      <div className="flex items-center justify-between border-t border-outline-variant/70 px-4 py-2">
        <div
          aria-live="polite"
          className="flex min-w-0 items-center gap-2 font-mono text-label-caps text-on-surface-variant"
          role="status"
        >
          <StatusPill icon={testStatus.icon} tone={testStatus.tone}>
            {testStatus.label}
          </StatusPill>
          <span className="truncate" title={testStatus.detail}>
            {testStatus.detail}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Button
            variant="ghost"
            onClick={() =>
              navigate({ to: isEditing ? "/query" : "/connections/new" })
            }
          >
            {isEditing ? "Cancel" : "← Back to Catalog"}
          </Button>
          {isEditing ? null : (
            <Button variant="secondary">Save as Draft</Button>
          )}
          <Button disabled={busy} variant="secondary" onClick={runTest}>
            {busy ? "Testing…" : "Test Connection"}
          </Button>
          <Button
            disabled={busy}
            variant="primary"
            icon="check"
            onClick={requestSaveChanges}
          >
            {isEditing ? "Save Changes" : "Save & Connect"}
          </Button>
        </div>
      </div>
      <AlertDialog
        onOpenChange={setReconnectPromptOpen}
        open={reconnectPromptOpen}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Reconnect with these changes?</AlertDialogTitle>
            <AlertDialogDescription>
              This connection is currently active. You can reconnect now to
              apply the updated settings, or save them and keep the current
              session running. Saved settings will apply the next time you
              connect.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel
              disabled={busy}
              onClick={() => setReconnectPromptOpen(false)}
            >
              Cancel
            </AlertDialogCancel>
            <Button
              disabled={busy}
              onClick={() => {
                setReconnectPromptOpen(false);
                void saveAndConnect(false);
              }}
              variant="secondary"
            >
              Save only
            </Button>
            <AlertDialogAction
              disabled={busy}
              onClick={() => {
                setReconnectPromptOpen(false);
                void saveAndConnect(true);
              }}
            >
              Save and reconnect
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function AuthenticationPanel({
  engine,
  method,
  passwordPlaceholder,
  onMethodChange,
  userName,
  password,
  onUserNameChange,
  onPasswordChange,
}: {
  engine: string;
  method: AuthenticationMethod;
  passwordPlaceholder?: string;
  onMethodChange: (method: AuthenticationMethod) => void;
  userName: string;
  password: string;
  onUserNameChange: (value: string) => void;
  onPasswordChange: (value: string) => void;
}) {
  const options = authenticationOptionsFor(engine);
  const selectedOption =
    options.find((option) => option.value === method) ?? options[0];

  return (
    <PropertySection title="Authentication">
      <PropertyRow label="Method" htmlFor="authentication-method">
        <Select
          value={method}
          onValueChange={(value) => {
            if (options.some((option) => option.value === value)) {
              onMethodChange(value as AuthenticationMethod);
            }
          }}
        >
          <SelectTrigger
            className="h-8 w-full text-body-sm"
            id="authentication-method"
          >
            <SelectValue>{selectedOption.label}</SelectValue>
          </SelectTrigger>
          <SelectContent align="start">
            {options.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </PropertyRow>
      {method === "windowsNtlm" ? (
        <>
          <div className="flex items-start gap-2 py-1 text-label-caps text-on-surface-variant">
            <Icon
              className="mt-0.5 shrink-0 text-warning"
              name="warning"
              size={13}
            />
            <span>
              NTLM uses your Active Directory username and password; a domain is
              optional.
            </span>
          </div>
          <div className="grid gap-2 md:grid-cols-2">
            <CredentialField
              id="active-directory-username"
              label="AD username"
              icon="user"
              value={userName}
              onChange={onUserNameChange}
            />
            <CredentialField
              id="active-directory-password"
              label="AD password"
              icon="lock"
              type="password"
              passwordPlaceholder={passwordPlaceholder}
              value={password}
              onChange={onPasswordChange}
            />
          </div>
        </>
      ) : method === "password" ? (
        <div className="grid gap-2 md:grid-cols-2">
          <CredentialField
            id="connection-username"
            label={
              engine === "Microsoft SQL Server" ? "SQL username" : "Username"
            }
            icon="user"
            value={userName}
            onChange={onUserNameChange}
          />
          <CredentialField
            id="connection-password"
            label="Password"
            icon="lock"
            type="password"
            passwordPlaceholder={passwordPlaceholder}
            value={password}
            onChange={onPasswordChange}
          />
        </div>
      ) : (
        <p className="text-label-caps text-on-surface-variant">
          Select a supported authentication method to continue.
        </p>
      )}
    </PropertySection>
  );
}

function DriverPropertiesPanel({
  engine,
  properties,
  onPropertiesChange,
}: {
  engine: string;
  properties: Readonly<Record<string, string>>;
  onPropertiesChange: (properties: Record<string, string>) => void;
}) {
  const isSqlServer = engine === "Microsoft SQL Server";
  const fields: DriverPropertyDefinition[] = isSqlServer
    ? [
        {
          key: "applicationName",
          label: "Application name",
          defaultValue: "Orbit SQL",
          kind: "text",
          description: "Name reported by the client connection.",
        },
        {
          key: "connectTimeoutSeconds",
          label: "Connect timeout (s)",
          defaultValue: "15",
          kind: "text",
          description: "Maximum time allowed to establish the connection.",
        },
        {
          key: "requestTimeoutSeconds",
          label: "Request timeout (s)",
          defaultValue: "15",
          kind: "text",
          description: "Maximum time allowed for a database request.",
        },
        {
          key: "packetSize",
          label: "Packet size",
          defaultValue: "4096",
          kind: "text",
          description: "TDS packet size requested by the driver.",
        },
        {
          key: "multiSubnetFailover",
          label: "Multi-subnet failover",
          defaultValue: "false",
          kind: "boolean",
          description: "Enables faster failover for multi-subnet deployments.",
        },
        {
          key: "encrypt",
          label: "Encrypt connection",
          defaultValue: "true",
          kind: "boolean",
          description: "Requires encryption for the SQL Server connection.",
        },
        {
          key: "fallbackToDefaultDb",
          label: "Fallback to default database",
          defaultValue: "false",
          kind: "boolean",
          description: "Retries using the server default database if needed.",
        },
        {
          key: "useUtc",
          label: "Use UTC timestamps",
          defaultValue: "true",
          kind: "boolean",
          description: "Maps timestamp values using UTC in the client.",
        },
        {
          key: "rowCollectionOnDone",
          label: "Collect rows on completion",
          defaultValue: "false",
          kind: "boolean",
          description:
            "Keeps collected rows available after request completion.",
        },
      ]
    : [
        {
          key: "applicationName",
          label: "Application name",
          defaultValue: "Orbit SQL",
          kind: "text",
          description: "Name reported by the client connection.",
        },
        {
          key: "connectTimeoutSeconds",
          label: "Connect timeout (s)",
          defaultValue: "15",
          kind: "text",
          description: "Maximum time allowed to establish the connection.",
        },
        {
          key: "statementTimeoutMilliseconds",
          label: "Statement timeout (ms)",
          defaultValue: "0",
          kind: "text",
          description:
            "Server-side limit for statement execution; zero disables it.",
        },
        {
          key: "statementCacheCapacity",
          label: "Statement cache capacity",
          defaultValue: "100",
          kind: "text",
          description: "Number of prepared statements retained by the client.",
        },
        {
          key: "sslMode",
          label: "SSL mode",
          defaultValue: "prefer",
          kind: "select",
          description: "TLS negotiation and certificate verification policy.",
          options: [
            { value: "disable", label: "Disable" },
            { value: "prefer", label: "Prefer" },
            { value: "require", label: "Require" },
            { value: "verifyCa", label: "Verify CA" },
            { value: "verifyFull", label: "Verify full" },
          ],
        },
        {
          key: "extraFloatDigits",
          label: "Extra float digits",
          defaultValue: "3",
          kind: "text",
          description: "Precision requested for floating-point output values.",
        },
        {
          key: "searchPath",
          label: "Search path",
          defaultValue: "",
          kind: "text",
          description: "Initial schema resolution path for unqualified names.",
        },
      ];

  const [filter, setFilter] = useState("");
  const [addingProperty, setAddingProperty] = useState(false);
  const [newPropertyName, setNewPropertyName] = useState("");
  const [newPropertyValue, setNewPropertyValue] = useState("");
  const [propertyError, setPropertyError] = useState<string | null>(null);
  const [removedKeys, setRemovedKeys] = useState<string[]>([]);
  const knownKeys = new Set(fields.map((field) => field.key));
  const rows = [
    ...fields
      .filter((field) => !removedKeys.includes(field.key))
      .map((field) => ({
        ...field,
        custom: false,
        value: properties[field.key] ?? field.defaultValue,
      })),
    ...Object.entries(properties)
      .filter(([key]) => !knownKeys.has(key))
      .map(([key, value]) => ({
        custom: true,
        defaultValue: "",
        description: "Custom driver property.",
        key,
        kind: "text" as const,
        label: key,
        value,
      })),
  ];
  const normalizedFilter = filter.trim().toLowerCase();
  const visibleRows = normalizedFilter
    ? rows.filter((row) =>
        `${row.key} ${row.label} ${row.value} ${row.description}`
          .toLowerCase()
          .includes(normalizedFilter),
      )
    : rows;

  function setProperty(key: string, value: string) {
    onPropertiesChange({ ...properties, [key]: value });
  }

  function removeProperty(key: string) {
    const nextProperties = { ...properties };
    delete nextProperties[key];
    onPropertiesChange(nextProperties);
    if (knownKeys.has(key)) {
      setRemovedKeys((current) =>
        current.includes(key) ? current : [...current, key],
      );
    }
  }

  function resetProperties() {
    onPropertiesChange(defaultDriverPropertiesFor(engine));
    setRemovedKeys([]);
    setPropertyError(null);
  }

  function openAddProperty() {
    setAddingProperty(true);
    setNewPropertyName("");
    setNewPropertyValue("");
    setPropertyError(null);
  }

  function closeAddProperty() {
    setAddingProperty(false);
    setNewPropertyName("");
    setNewPropertyValue("");
    setPropertyError(null);
  }

  function addProperty() {
    const key = newPropertyName.trim();
    if (!key) {
      setPropertyError("Property name is required.");
      return;
    }
    const keyAlreadyVisible =
      (knownKeys.has(key) && !removedKeys.includes(key)) ||
      (!knownKeys.has(key) && Object.keys(properties).includes(key));
    if (keyAlreadyVisible) {
      setPropertyError("A property with this name already exists.");
      return;
    }
    onPropertiesChange({ ...properties, [key]: newPropertyValue });
    setRemovedKeys((current) =>
      current.filter((currentKey) => currentKey !== key),
    );
    closeAddProperty();
  }

  return (
    <PropertySection
      title={`${engine} driver properties`}
      className="max-w-none"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-label-caps text-on-surface-variant">
          {rows.length} properties · values are saved with this connection.
        </p>
        <div className="flex items-center gap-1">
          <Button
            className="h-7 px-2 text-label-caps"
            icon="plus"
            onClick={openAddProperty}
          >
            Add property
          </Button>
          <IconButton
            className="h-7 w-7"
            label="Reset driver properties"
            name="refresh"
            onClick={resetProperties}
          />
        </div>
      </div>
      <InputGroup className="h-7 max-w-xs">
        <InputGroupAddon align="inline-start">
          <Icon className="text-control-icon" name="search" size={13} />
        </InputGroupAddon>
        <InputGroupInput
          aria-label="Filter driver properties"
          className="text-label-caps"
          onChange={(event) => setFilter(event.target.value)}
          placeholder="Filter properties"
          value={filter}
        />
      </InputGroup>
      <div className="overflow-x-auto rounded-md border border-control-border bg-surface-raised">
        <table className="w-full min-w-[42rem] table-fixed border-collapse text-left">
          <thead>
            <tr className="border-b border-surface-divider bg-surface-subtle">
              <th className="w-[28%] px-2.5 py-1.5 text-label-caps font-semibold text-on-surface-variant">
                Name
              </th>
              <th className="w-[28%] px-2.5 py-1.5 text-label-caps font-semibold text-on-surface-variant">
                Value
              </th>
              <th className="px-2.5 py-1.5 text-label-caps font-semibold text-on-surface-variant">
                Description
              </th>
              <th className="w-10 px-1 py-1.5" />
            </tr>
          </thead>
          <tbody>
            {addingProperty ? (
              <tr className="border-b border-surface-divider bg-selected-surface/40">
                <td className="px-2 py-1.5">
                  <Input
                    autoCapitalize="none"
                    autoCorrect="off"
                    autoFocus
                    className="h-7 rounded-sm px-2 font-mono text-label-caps"
                    onChange={(event) => setNewPropertyName(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") addProperty();
                      if (event.key === "Escape") closeAddProperty();
                    }}
                    placeholder="propertyName"
                    spellCheck={false}
                    value={newPropertyName}
                  />
                </td>
                <td className="px-2 py-1.5">
                  <Input
                    autoCapitalize="none"
                    autoCorrect="off"
                    className="h-7 rounded-sm px-2 font-mono text-label-caps"
                    onChange={(event) =>
                      setNewPropertyValue(event.target.value)
                    }
                    onKeyDown={(event) => {
                      if (event.key === "Enter") addProperty();
                      if (event.key === "Escape") closeAddProperty();
                    }}
                    placeholder="value"
                    spellCheck={false}
                    value={newPropertyValue}
                  />
                </td>
                <td className="px-2.5 py-1.5 text-label-caps text-on-surface-variant">
                  New custom driver property
                </td>
                <td className="px-1 py-1.5">
                  <div className="flex items-center gap-0.5">
                    <IconButton
                      className="h-6 w-6 text-primary"
                      label="Add property"
                      name="check"
                      onClick={addProperty}
                    />
                    <IconButton
                      className="h-6 w-6"
                      label="Cancel adding property"
                      name="close"
                      onClick={closeAddProperty}
                    />
                  </div>
                </td>
              </tr>
            ) : null}
            {visibleRows.map((row) => (
              <tr
                className="border-b border-surface-divider last:border-b-0 hover:bg-control-surface-hover"
                key={row.key}
              >
                <td className="max-w-0 px-2.5 py-1.5 align-middle">
                  <span
                    className="block truncate font-mono text-label-caps text-on-surface"
                    title={row.label}
                  >
                    {row.key}
                  </span>
                </td>
                <td className="px-2 py-1.5 align-middle">
                  <DriverPropertyValue
                    id={`driver-${row.key}`}
                    property={row}
                    value={row.value}
                    onChange={(value) => setProperty(row.key, value)}
                  />
                </td>
                <td
                  className="max-w-0 px-2.5 py-1.5 align-middle text-label-caps text-on-surface-variant"
                  title={row.description}
                >
                  <span className="block truncate">{row.description}</span>
                </td>
                <td className="px-1 py-1.5 align-middle">
                  <IconButton
                    className="h-6 w-6 text-on-surface-variant hover:text-error"
                    label={`Delete ${row.key}`}
                    name="trash"
                    onClick={() => removeProperty(row.key)}
                  />
                </td>
              </tr>
            ))}
            {visibleRows.length === 0 && !addingProperty ? (
              <tr>
                <td
                  className="px-2.5 py-6 text-center text-label-caps text-on-surface-variant"
                  colSpan={4}
                >
                  No driver properties match this filter.
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
      {propertyError ? (
        <p className="text-label-caps text-error" role="alert">
          {propertyError}
        </p>
      ) : null}
    </PropertySection>
  );
}

function DriverPropertyValue({
  id,
  property,
  value,
  onChange,
}: {
  id: string;
  property: DriverPropertyDefinition;
  value: string;
  onChange: (value: string) => void;
}) {
  if (property.kind === "boolean") {
    return (
      <Select
        value={value}
        onValueChange={(nextValue) => onChange(nextValue ?? value)}
      >
        <SelectTrigger
          className="h-7 w-full min-w-0 rounded-sm px-2 text-label-caps"
          id={id}
        >
          <SelectValue>{value === "true" ? "Enabled" : "Disabled"}</SelectValue>
        </SelectTrigger>
        <SelectContent align="start">
          <SelectItem value="true">Enabled</SelectItem>
          <SelectItem value="false">Disabled</SelectItem>
        </SelectContent>
      </Select>
    );
  }

  if (property.kind === "select") {
    return (
      <Select
        value={value}
        onValueChange={(nextValue) => onChange(nextValue ?? value)}
      >
        <SelectTrigger
          className="h-7 w-full min-w-0 rounded-sm px-2 text-label-caps"
          id={id}
        >
          <SelectValue>
            {property.options?.find((option) => option.value === value)
              ?.label ?? value}
          </SelectValue>
        </SelectTrigger>
        <SelectContent align="start">
          {property.options?.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  }

  return (
    <Input
      autoCapitalize="none"
      autoCorrect="off"
      className="h-7 rounded-sm px-2 font-mono text-label-caps"
      id={id}
      onChange={(event) => onChange(event.target.value)}
      spellCheck={false}
      value={value}
    />
  );
}

function TransportFields({
  transport,
  onChange,
}: {
  transport: ConnectionTransport;
  onChange: (transport: ConnectionTransport) => void;
}) {
  if (transport.mode === "direct") {
    return (
      <p className="text-label-caps text-on-surface-variant">
        Connect directly to the database host and port.
      </p>
    );
  }

  if (transport.mode === "ssh") {
    return (
      <div className="grid gap-2 md:grid-cols-2">
        <PropertyRow label="SSH host" htmlFor="ssh-host">
          <ConnectionInput
            id="ssh-host"
            prefix="dns"
            value={transport.sshHost}
            onChange={(value) => onChange({ ...transport, sshHost: value })}
          />
        </PropertyRow>
        <PropertyRow label="SSH port" htmlFor="ssh-port">
          <ConnectionInput
            id="ssh-port"
            value={String(transport.sshPort)}
            onChange={(value) =>
              onChange({ ...transport, sshPort: Number(value) })
            }
          />
        </PropertyRow>
        <PropertyRow label="SSH user" htmlFor="ssh-user">
          <ConnectionInput
            id="ssh-user"
            value={transport.sshUserName}
            onChange={(value) => onChange({ ...transport, sshUserName: value })}
          />
        </PropertyRow>
        <PropertyRow label="Private key" htmlFor="ssh-private-key">
          <ConnectionInput
            id="ssh-private-key"
            value={transport.sshPrivateKeyPath}
            onChange={(value) =>
              onChange({ ...transport, sshPrivateKeyPath: value })
            }
          />
        </PropertyRow>
      </div>
    );
  }

  return (
    <div className="grid gap-2 md:grid-cols-[minmax(0,1fr)_minmax(8rem,0.4fr)]">
      <PropertyRow label="Proxy host" htmlFor="proxy-host">
        <ConnectionInput
          id="proxy-host"
          prefix="dns"
          value={transport.proxyHost}
          onChange={(value) => onChange({ ...transport, proxyHost: value })}
        />
      </PropertyRow>
      <PropertyRow label="Proxy port" htmlFor="proxy-port">
        <ConnectionInput
          id="proxy-port"
          value={String(transport.proxyPort)}
          onChange={(value) =>
            onChange({ ...transport, proxyPort: Number(value) })
          }
        />
      </PropertyRow>
      <PropertyRow label="Proxy protocol" htmlFor="proxy-type">
        <Select
          value={transport.proxyType}
          onValueChange={(value) => {
            if (value === "socks5" || value === "httpConnect") {
              onChange({ ...transport, proxyType: value });
            }
          }}
        >
          <SelectTrigger className="h-8 w-full text-body-sm" id="proxy-type">
            <SelectValue>
              {transport.proxyType === "socks5" ? "SOCKS5" : "HTTP CONNECT"}
            </SelectValue>
          </SelectTrigger>
          <SelectContent align="start">
            <SelectItem value="socks5">SOCKS5</SelectItem>
            <SelectItem value="httpConnect">HTTP CONNECT</SelectItem>
          </SelectContent>
        </Select>
      </PropertyRow>
      <p className="text-label-caps text-on-surface-variant md:col-span-2">
        The proxy must allow unauthenticated TCP tunneling to the database.
      </p>
    </div>
  );
}

function CredentialField({
  id,
  icon,
  label,
  passwordPlaceholder,
  type = "text",
  value,
  onChange,
}: {
  id: string;
  icon: "file" | "key" | "lock" | "user";
  label: string;
  passwordPlaceholder?: string;
  type?: ComponentProps<"input">["type"];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <PropertyRow label={label} htmlFor={id}>
      <InputGroup className="h-8">
        <InputGroupAddon align="inline-start">
          <Icon className="text-control-icon" name={icon} size={13} />
        </InputGroupAddon>
        <InputGroupInput
          autoCapitalize="none"
          autoCorrect="off"
          className="text-body-sm"
          id={id}
          placeholder={
            type === "password"
              ? (passwordPlaceholder ?? "Enter password")
              : undefined
          }
          spellCheck={false}
          type={type}
          onChange={(event) => onChange(event.target.value)}
          value={value}
        />
      </InputGroup>
    </PropertyRow>
  );
}

function ConnectionInput({
  id,
  value,
  prefix,
  placeholder,
  onChange,
}: {
  id: string;
  value: string;
  prefix?: "dns" | "database";
  placeholder?: string;
  onChange: (value: string) => void;
}) {
  return (
    <InputGroup className="h-8">
      {prefix ? (
        <InputGroupAddon align="inline-start">
          <Icon
            name={prefix === "dns" ? "network" : "database"}
            size={12}
            className="text-control-icon"
          />
        </InputGroupAddon>
      ) : null}
      <InputGroupInput
        autoCapitalize="none"
        autoCorrect="off"
        className="font-mono text-label-caps"
        id={id}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        spellCheck={false}
        value={value}
      />
    </InputGroup>
  );
}

function ProfilePanel({
  activeProfileId,
  onActiveProfileChange,
  onProfilesChange,
  profiles,
}: {
  activeProfileId: string;
  onActiveProfileChange: (profileId: string) => void;
  onProfilesChange: (profiles: ConnectionProfileDefinition[]) => void;
  profiles: ConnectionProfileDefinition[];
}) {
  const activeProfile = profiles.find(
    (profile) => profile.id === activeProfileId,
  );
  const [formOpen, setFormOpen] = useState(false);
  const [editingProfileId, setEditingProfileId] = useState<string | null>(null);
  const [profileName, setProfileName] = useState("");
  const [profileColor, setProfileColor] = useState<string>(
    profileColors[0].value,
  );
  const [profileBusy, setProfileBusy] = useState(false);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [manageOpen, setManageOpen] = useState(false);

  function openCreateForm() {
    setEditingProfileId(null);
    setProfileName("");
    setProfileColor(profileColors[0].value);
    setProfileError(null);
    setFormOpen(true);
  }

  function openEditForm(profile: ConnectionProfileDefinition) {
    setEditingProfileId(profile.id);
    setProfileName(profile.name);
    setProfileColor(profileColorHex(profile.color));
    setProfileError(null);
    setFormOpen(true);
  }

  async function saveProfile() {
    const name = profileName.trim();
    if (!name) return;
    setProfileBusy(true);
    setProfileError(null);
    try {
      const savedProfile = await upsertConnectionProfileDefinition({
        id: editingProfileId ?? crypto.randomUUID(),
        name,
        color: profileColor,
        description: editingProfileId
          ? (profiles.find((profile) => profile.id === editingProfileId)
              ?.description ?? "Connection profile.")
          : "Connection profile.",
      });
      const nextProfiles = editingProfileId
        ? profiles.map((profile) =>
            profile.id === editingProfileId ? savedProfile : profile,
          )
        : [...profiles, savedProfile];
      onProfilesChange(nextProfiles);
      if (!editingProfileId) onActiveProfileChange(savedProfile.id);
      for (const connection of Object.values(
        useConnectionsStore.getState().connections,
      )) {
        if (connection.profileId !== savedProfile.id) continue;
        const database = connection.detail.split(" • ")[1] ?? "";
        useConnectionsStore.getState().updateConnection(connection.id, {
          profileColor: savedProfile.color,
          detail: connectionDetail(savedProfile.id, database, nextProfiles),
        });
      }
      setFormOpen(false);
    } catch (error) {
      setProfileError(connectionErrorMessage(error));
    } finally {
      setProfileBusy(false);
    }
  }

  async function deleteProfile(profile: ConnectionProfileDefinition) {
    if (profiles.length <= 1) return;
    setProfileBusy(true);
    setProfileError(null);
    try {
      const connections = await listSavedConnections();
      const isAssigned = connections.some(
        (connection) =>
          connection.profileId === profile.id ||
          connection.profileId.toLowerCase() === profile.name.toLowerCase(),
      );
      if (isAssigned) {
        setProfileError(`${profile.name} is assigned to saved connections.`);
        return;
      }
      await deleteConnectionProfileDefinition(profile.id);
      const remainingProfiles = profiles.filter(
        (candidate) => candidate.id !== profile.id,
      );
      onProfilesChange(remainingProfiles);
      if (activeProfileId === profile.id) {
        onActiveProfileChange(remainingProfiles[0]?.id ?? "");
      }
      if (editingProfileId === profile.id) setFormOpen(false);
    } catch (error) {
      setProfileError(connectionErrorMessage(error));
    } finally {
      setProfileBusy(false);
    }
  }

  return (
    <>
      {profiles.length > 0 && activeProfile ? (
        <div className="flex min-w-0 items-center gap-1.5">
          <Select
            value={activeProfile.id}
            onValueChange={(value) => {
              if (value) onActiveProfileChange(value);
            }}
          >
            <SelectTrigger
              className="h-8 min-w-0 flex-1 text-body-sm"
              id="active-profile-select"
            >
              <span className="flex min-w-0 flex-1 items-center gap-2">
                <span className="truncate">{activeProfile.name}</span>
                <ProfileColorDot
                  className="ml-auto"
                  color={activeProfile.color}
                />
              </span>
            </SelectTrigger>
            <SelectContent align="start">
              {profiles.map((profile) => (
                <SelectItem
                  className="pr-14"
                  key={profile.id}
                  value={profile.id}
                >
                  <span className="flex min-w-0 flex-1 items-center gap-2">
                    <span className="truncate">{profile.name}</span>
                    <ProfileColorDot
                      className="absolute right-8"
                      color={profile.color}
                    />
                  </span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            className="h-8 shrink-0 px-2"
            onClick={() => setManageOpen(true)}
            variant="ghost"
          >
            Manage…
          </Button>
        </div>
      ) : (
        <span className="text-label-caps text-on-surface-variant">
          Loading profiles…
        </span>
      )}
      <Dialog
        open={manageOpen}
        onOpenChange={(open) => {
          setManageOpen(open);
          if (!open) {
            setFormOpen(false);
            setProfileError(null);
          }
        }}
      >
        <DialogContent className="w-[min(600px,calc(100vw-2rem))] sm:max-w-[600px]">
          <DialogHeader>
            <DialogTitle>Profiles</DialogTitle>
            <DialogDescription>
              Manage the profiles available for connections.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="flex items-center justify-between gap-3">
              <p className="text-label-caps text-on-surface-variant">
                {profiles.length}{" "}
                {profiles.length === 1 ? "profile" : "profiles"}
              </p>
              <Button
                className="h-8 px-2"
                disabled={profileBusy}
                icon="plus"
                onClick={openCreateForm}
                variant="secondary"
              >
                New profile
              </Button>
            </div>
            <div className="max-h-[min(42vh,360px)] space-y-1.5 overflow-y-auto pr-1">
              {profiles.map((profile) => {
                const selected = profile.id === activeProfileId;
                const editing = profile.id === editingProfileId && formOpen;
                return (
                  <div
                    className={`flex items-center gap-2 rounded-md border p-2 ${editing ? "border-primary/45 bg-selected-surface/60" : selected ? "border-primary/25 bg-selected-surface/35" : "border-surface-divider bg-surface-subtle"}`}
                    key={profile.id}
                  >
                    <button
                      aria-pressed={selected}
                      className="flex min-w-0 flex-1 items-center gap-2 rounded-sm px-1 py-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-primary"
                      onClick={() => {
                        onActiveProfileChange(profile.id);
                        setFormOpen(false);
                        setProfileError(null);
                      }}
                      type="button"
                    >
                      <ProfileColorDot color={profile.color} />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-body-sm font-medium text-on-surface">
                          {profile.name}
                        </span>
                        <span className="block truncate text-label-caps text-on-surface-variant">
                          {profile.description}
                        </span>
                      </span>
                      {selected ? (
                        <span className="shrink-0 text-label-caps text-primary">
                          Active
                        </span>
                      ) : null}
                    </button>
                    <div className="flex shrink-0 items-center gap-1">
                      <Button
                        aria-label={`Edit ${profile.name}`}
                        className="h-7 px-2"
                        disabled={profileBusy}
                        icon="settings"
                        onClick={() => openEditForm(profile)}
                        variant="ghost"
                      >
                        Edit
                      </Button>
                      <Button
                        aria-label={`Delete ${profile.name}`}
                        className="h-7 px-2"
                        disabled={profileBusy || profiles.length <= 1}
                        icon="trash"
                        onClick={() => void deleteProfile(profile)}
                        variant="danger"
                      >
                        Delete
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
            {formOpen ? (
              <div className="space-y-2 rounded-md border border-surface-divider p-3">
                <h3 className="text-body-sm font-medium text-on-surface">
                  {editingProfileId ? "Edit profile" : "New profile"}
                </h3>
                <PropertyRow label="Profile name" htmlFor="profile-name">
                  <Input
                    autoCapitalize="none"
                    autoCorrect="off"
                    className="h-8"
                    id="profile-name"
                    onChange={(event) => setProfileName(event.target.value)}
                    placeholder="e.g. Staging"
                    spellCheck={false}
                    value={profileName}
                  />
                </PropertyRow>
                <PropertyRow label="Color" htmlFor="profile-color-picker">
                  <div className="flex h-8 items-center gap-2 rounded-md border border-surface-divider bg-surface-subtle px-2">
                    {profileColors.map((color) => (
                      <button
                        aria-label={`${color.label} profile color`}
                        aria-pressed={profileColor === color.value}
                        className="flex size-5 items-center justify-center rounded-full outline-none ring-offset-2 ring-offset-surface-subtle focus-visible:ring-2 focus-visible:ring-primary"
                        key={color.value}
                        onClick={() => setProfileColor(color.value)}
                        style={{ backgroundColor: color.value }}
                        title={color.label}
                        type="button"
                      >
                        {profileColor === color.value ? (
                          <span className="size-1.5 rounded-full bg-surface-floating" />
                        ) : null}
                      </button>
                    ))}
                    <span
                      aria-hidden="true"
                      className="mx-0.5 h-4 w-px bg-surface-divider"
                    />
                    <label className="flex h-6 cursor-pointer items-center gap-1.5 rounded px-1.5 text-label-caps text-on-surface-variant hover:bg-control-surface-hover">
                      <input
                        aria-label="Choose a custom profile color"
                        className="size-5 cursor-pointer rounded border-0 bg-transparent p-0"
                        id="profile-color-picker"
                        onChange={(event) =>
                          setProfileColor(event.target.value)
                        }
                        type="color"
                        value={profileColorHex(profileColor)}
                      />
                      <span>Custom</span>
                    </label>
                  </div>
                </PropertyRow>
              </div>
            ) : null}
            {profileError ? (
              <p className="text-label-caps text-error">{profileError}</p>
            ) : null}
          </div>
          {formOpen ? (
            <DialogFooter>
              <Button
                disabled={profileBusy}
                onClick={() => setFormOpen(false)}
                variant="ghost"
              >
                Cancel
              </Button>
              <Button
                disabled={profileBusy || !profileName.trim()}
                onClick={saveProfile}
                variant="primary"
              >
                Save profile
              </Button>
            </DialogFooter>
          ) : null}
        </DialogContent>
      </Dialog>
    </>
  );
}
