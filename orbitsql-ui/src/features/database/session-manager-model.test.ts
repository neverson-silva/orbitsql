import { describe, expect, it, vi } from "vitest";
import type { DatabaseSession } from "../connections/connection-ipc";
import {
  createSessionRequestGate,
  filterAndSortSessions,
  isSessionPermissionError,
  type SessionFilter,
} from "./session-manager-model";

function makeSession(
  sessionId: number,
  changes: Partial<DatabaseSession> = {},
): DatabaseSession {
  return {
    sessionId,
    status: "sleeping",
    requestStatus: null,
    database: "appdb",
    login: "app_user",
    originalLogin: "app_user",
    host: "db-client",
    application: "Orbit SQL",
    clientInterface: "tedious-rs",
    clientAddress: "127.0.0.1",
    localAddress: "127.0.0.1",
    localPort: 1433,
    loginTime: "2026-10-06T00:00:00",
    command: null,
    durationMs: "1000",
    cpuTimeMs: 0,
    reads: "0",
    writes: "0",
    logicalReads: "0",
    rowCount: "0",
    waitType: null,
    waitTimeMs: 0,
    waitResource: null,
    lastWaitType: null,
    blockingSessionId: null,
    blockerSessionId: null,
    rootBlockerSessionId: null,
    isBlocking: false,
    isRootBlocker: false,
    blockedSessionCount: 0,
    blockingDepth: 0,
    openTransactionCount: 0,
    transactionIsolationLevel: 2,
    transactionIsolationName: "Read committed",
    batchText: null,
    currentStatement: null,
    percentComplete: 0,
    estimatedCompletionTimeMs: "0",
    ...changes,
  };
}

const sessions = [
  makeSession(51, {
    status: "running",
    requestStatus: "running",
    durationMs: "9007199254740993",
    login: "billing_user",
    command: "SELECT",
    batchText: "SELECT * FROM invoices",
  }),
  makeSession(52, {
    status: "suspended",
    requestStatus: "suspended",
    durationMs: "8000",
    blockingSessionId: 51,
    waitType: "LCK_M_S",
  }),
  makeSession(54, { durationMs: "9007199254740992" }),
  makeSession(53, { durationMs: "900" }),
];

function filtered(
  filter: SessionFilter,
  search = "",
  sortKey: "sessionId" | "durationMs" = "sessionId",
  descending = false,
) {
  return filterAndSortSessions(sessions, filter, search, {
    key: sortKey,
    descending,
  });
}

describe("session manager filtering and ordering", () => {
  it("renders all sessions and sorts by the raw duration value", () => {
    expect(
      filtered("all", "", "durationMs").map((session) => session.sessionId),
    ).toEqual([53, 52, 54, 51]);
    expect(
      filtered("all", "", "durationMs", true).map(
        (session) => session.sessionId,
      ),
    ).toEqual([51, 54, 52, 53]);
  });

  it("prioritizes blockers, active requests, and then idle sessions", () => {
    const records = [
      makeSession(70, {
        isRootBlocker: true,
        isBlocking: true,
        durationMs: "500",
      }),
      makeSession(71, { requestStatus: "running", durationMs: "10000" }),
      makeSession(72, { blockingSessionId: 70, requestStatus: "suspended" }),
      makeSession(73, { durationMs: "90000" }),
    ];
    expect(
      filterAndSortSessions(records, "all", "", {
        key: "priority",
        descending: false,
      }).map((session) => session.sessionId),
    ).toEqual([70, 72, 71, 73]);
  });

  it("filters active, blocked, blocking, long-running, and idle sessions", () => {
    expect(filtered("active").map((session) => session.sessionId)).toEqual([
      51, 52,
    ]);
    expect(filtered("blocked").map((session) => session.sessionId)).toEqual([
      52,
    ]);
    expect(filtered("blocking").map((session) => session.sessionId)).toEqual(
      [],
    );
    const withBlocker = sessions.map((session) =>
      session.sessionId === 51
        ? { ...session, isBlocking: true, isRootBlocker: true }
        : session,
    );
    expect(
      filterAndSortSessions(withBlocker, "blocking", "", {
        key: "sessionId",
        descending: false,
      }).map((session) => session.sessionId),
    ).toEqual([51]);
    expect(filtered("long").map((session) => session.sessionId)).toEqual([
      51, 54,
    ]);
    expect(filtered("idle").map((session) => session.sessionId)).toEqual([
      53, 54,
    ]);
  });

  it("searches session ids, login, host, application, command, wait, and SQL text", () => {
    expect(
      filtered("all", "billing_user").map((session) => session.sessionId),
    ).toEqual([51]);
    expect(
      filtered("all", "lck_m_s").map((session) => session.sessionId),
    ).toEqual([52]);
    expect(
      filtered("all", "invoices").map((session) => session.sessionId),
    ).toEqual([51]);
    expect(filtered("all", "53").map((session) => session.sessionId)).toEqual([
      53,
    ]);
  });
});

describe("session manager request gate", () => {
  it("skips overlapping telemetry requests for the same connection", async () => {
    const gate = createSessionRequestGate();
    let release: () => void = () => {};
    const firstValue = {
      sessions: [],
      monitorSessionId: 10,
      capturedAt: "first",
    };
    const firstRequest = vi.fn(
      () =>
        new Promise<typeof firstValue>((resolve) => {
          release = () => resolve(firstValue);
        }),
    );
    const first = gate.run("connection-a", firstRequest);

    const secondValue = {
      sessions: [],
      monitorSessionId: 11,
      capturedAt: "second",
    };
    const secondRequest = vi.fn(async () => secondValue);
    const second = gate.run("connection-a", secondRequest);
    release();

    await expect(first).resolves.toBe(firstValue);
    await expect(second).resolves.toBe(firstValue);
    expect(firstRequest).toHaveBeenCalledTimes(1);
    expect(secondRequest).not.toHaveBeenCalled();
    expect(gate.isRunning("connection-a")).toBe(false);
  });

  it("keeps different connection requests isolated", async () => {
    const gate = createSessionRequestGate();
    let release: () => void = () => {};
    const firstValue = { sessions: [], monitorSessionId: 10, capturedAt: "a" };
    const pending = gate.run(
      "connection-a",
      () =>
        new Promise<typeof firstValue>((resolve) => {
          release = () => resolve(firstValue);
        }),
    );

    const secondValue = { sessions: [], monitorSessionId: 11, capturedAt: "b" };
    await expect(
      gate.run("connection-b", async () => secondValue),
    ).resolves.toBe(secondValue);
    release();
    await expect(pending).resolves.toBe(firstValue);
  });
});

describe("session telemetry error classification", () => {
  it("recognizes permission failures without treating unrelated errors as permissions", () => {
    expect(
      isSessionPermissionError("SESSION_TELEMETRY_PERMISSION_DENIED: denied"),
    ).toBe(true);
    expect(isSessionPermissionError("DRIVER_ERROR: timeout")).toBe(false);
    expect(isSessionPermissionError(null)).toBe(false);
  });
});
