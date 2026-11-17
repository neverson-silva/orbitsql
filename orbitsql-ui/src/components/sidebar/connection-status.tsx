import {
  type ConnectionStatus as ConnectionStatusValue,
  isConnectionReady,
} from "../../stores/connections.store";
import { Icon } from "../ui/icons";

function statusCopy(
  status: ConnectionStatusValue,
  refreshing: boolean,
): string {
  if (refreshing || status === "introspecting") return "Introspecting…";
  if (status === "connecting") return "Connecting…";
  if (status === "disconnecting") return "Disconnecting…";
  if (status === "error") return "Connection failed";
  return isConnectionReady(status) ? "Connected" : "Disconnected";
}

export function ConnectionStatus({
  status,
  refreshing,
}: {
  status: ConnectionStatusValue;
  refreshing: boolean;
}) {
  const busy =
    refreshing ||
    status === "connecting" ||
    status === "introspecting" ||
    status === "disconnecting";
  const connected = isConnectionReady(status);
  const failed = status === "error";
  const label = statusCopy(status, refreshing);
  const icon = busy
    ? "spinner"
    : connected
      ? "plugs-connected"
      : failed
        ? "plug"
        : "plugs";
  const tone = busy
    ? "text-primary"
    : connected
      ? "text-success"
      : failed
        ? "text-error"
        : "text-on-surface-variant";

  return (
    <span
      aria-label={label}
      className={`flex size-7 shrink-0 items-center justify-center ${tone}`}
      role="img"
      title={label}
    >
      <Icon
        className={busy ? "animate-spin" : undefined}
        name={icon}
        size={15}
      />
    </span>
  );
}
