import type {
  DatabaseSession,
  SessionTelemetry,
} from "../connections/connection-ipc";

export const LONG_RUNNING_THRESHOLD_MS = 30_000;

export function isSessionPermissionError(error: string | null): boolean {
  return error?.includes("SESSION_TELEMETRY_PERMISSION_DENIED") ?? false;
}

export type SessionFilter =
  | "all"
  | "active"
  | "blocked"
  | "blocking"
  | "long"
  | "idle";

export type SessionSortKey =
  | "priority"
  | "sessionId"
  | "status"
  | "durationMs"
  | "database"
  | "login"
  | "host"
  | "command"
  | "cpuTimeMs"
  | "reads"
  | "writes"
  | "waitType"
  | "blockingSessionId"
  | "currentStatement";

export type SessionSort = Readonly<{
  key: SessionSortKey;
  descending: boolean;
}>;

export function filterAndSortSessions(
  sessions: readonly DatabaseSession[],
  filter: SessionFilter,
  search: string,
  sort: SessionSort,
): DatabaseSession[] {
  const query = search.trim().toLocaleLowerCase();
  return sessions
    .filter((session) => {
      if (filter === "active" && !session.requestStatus) return false;
      if (
        filter === "blocked" &&
        !(session.blockingSessionId && session.blockingSessionId !== 0)
      )
        return false;
      if (filter === "blocking" && !session.isBlocking) return false;
      if (
        filter === "long" &&
        compareBigIntegers(
          session.durationMs,
          String(LONG_RUNNING_THRESHOLD_MS),
        ) < 0
      )
        return false;
      if (filter === "idle" && session.requestStatus) return false;
      if (!query) return true;
      return [
        session.sessionId,
        session.database,
        session.login,
        session.host,
        session.application,
        session.command,
        session.waitType,
        session.batchText,
      ].some((value) =>
        String(value ?? "")
          .toLocaleLowerCase()
          .includes(query),
      );
    })
    .sort((left, right) => {
      if (sort.key === "priority") {
        const priority = (session: DatabaseSession) =>
          session.isRootBlocker
            ? 0
            : session.blockingSessionId !== null &&
                session.blockingSessionId !== 0
              ? 1
              : session.requestStatus
                ? 2
                : 3;
        const priorityDifference = priority(left) - priority(right);
        if (priorityDifference !== 0) return priorityDifference;
        return compareBigIntegers(right.durationMs, left.durationMs);
      }
      const a = left[sort.key];
      const b = right[sort.key];
      const comparison =
        sort.key === "durationMs" ||
        sort.key === "reads" ||
        sort.key === "writes"
          ? compareBigIntegers(String(a ?? "0"), String(b ?? "0"))
          : typeof a === "number" && typeof b === "number"
            ? a - b
            : String(a ?? "").localeCompare(String(b ?? ""), undefined, {
                sensitivity: "base",
              });
      return sort.descending ? -comparison : comparison;
    });
}

function compareBigIntegers(left: string, right: string) {
  try {
    const leftInteger = BigInt(left);
    const rightInteger = BigInt(right);
    return leftInteger < rightInteger ? -1 : leftInteger > rightInteger ? 1 : 0;
  } catch {
    return left.localeCompare(right, undefined, { numeric: true });
  }
}

export function createSessionRequestGate() {
  const requests = new Map<string, Promise<SessionTelemetry>>();
  return {
    isRunning(connectionId: string) {
      return requests.has(connectionId);
    },
    async run(
      connectionId: string,
      request: () => Promise<SessionTelemetry>,
    ): Promise<SessionTelemetry> {
      const existing = requests.get(connectionId);
      if (existing) return existing;
      const pending = request();
      requests.set(connectionId, pending);
      try {
        return await pending;
      } finally {
        if (requests.get(connectionId) === pending)
          requests.delete(connectionId);
      }
    },
  };
}
