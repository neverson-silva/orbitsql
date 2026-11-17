import { create } from "zustand";
import {
  closeQuerySession,
  type QueryResult,
} from "../features/connections/connection-ipc";
import type { ExecutionContext } from "../features/connections/execution-context";

export type QueryTab = {
  id: string;
  title: string;
  query: string;
  connectionId: string | null;
  executionContext: ExecutionContext;
  result: QueryResult | null;
  resultTab: string;
  running: boolean;
  errorMessage: string | null;
  lastExecutedQuery: string | null;
};

type QueryTabsState = {
  tabs: QueryTab[];
  activeTabId: string | null;
  openTab: (
    connectionId?: string | null,
    executionContext?: ExecutionContext,
  ) => string;
  closeTab: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;
  updateTab: (tabId: string, changes: Partial<QueryTab>) => void;
  setExecutionContext: (tabId: string, context: ExecutionContext) => void;
};

let queryNumber = 1;

function createQueryTab(
  connectionId: string | null,
  executionContext?: ExecutionContext,
): QueryTab {
  const id = `query-${crypto.randomUUID()}`;
  const title = `Query ${queryNumber}`;
  queryNumber += 1;
  return {
    id,
    title,
    query: "",
    connectionId,
    executionContext: executionContext ?? {
      connectionId,
      dimensions: {},
      connectionState: "disconnected",
    },
    result: null,
    resultTab: "Result View",
    running: false,
    errorMessage: null,
    lastExecutedQuery: null,
  };
}

export const useQueryTabsStore = create<QueryTabsState>((set) => ({
  tabs: [],
  activeTabId: null,
  openTab: (connectionId = null, executionContext) => {
    const tab = createQueryTab(connectionId, executionContext);
    set((state) => ({
      tabs: [...state.tabs, tab],
      activeTabId: tab.id,
    }));
    return tab.id;
  },
  closeTab: (tabId) => {
    let sessionId: string | null = null;
    set((state) => {
      const closedIndex = state.tabs.findIndex((tab) => tab.id === tabId);
      if (closedIndex === -1) return state;
      sessionId = state.tabs[closedIndex]?.result?.sessionId ?? null;
      const tabs = state.tabs.filter((tab) => tab.id !== tabId);
      if (state.activeTabId !== tabId) return { tabs };
      const nextTab = tabs[Math.min(closedIndex, tabs.length - 1)] ?? null;
      return { tabs, activeTabId: nextTab?.id ?? null };
    });
    if (sessionId) void closeQuerySession(sessionId);
  },
  setActiveTab: (tabId) =>
    set((state) =>
      state.tabs.some((tab) => tab.id === tabId)
        ? { activeTabId: tabId }
        : state,
    ),
  updateTab: (tabId, changes) =>
    set((state) => {
      const tab = state.tabs.find((item) => item.id === tabId);
      if (!tab) return state;
      return {
        tabs: state.tabs.map((item) =>
          item.id === tabId ? { ...item, ...changes } : item,
        ),
      };
    }),
  setExecutionContext: (tabId, executionContext) =>
    set((state) => ({
      tabs: state.tabs.map((tab) =>
        tab.id === tabId ? { ...tab, executionContext } : tab,
      ),
    })),
}));

export function openQueryTab(
  connectionId: string | null = null,
  executionContext?: ExecutionContext,
): string {
  return useQueryTabsStore.getState().openTab(connectionId, executionContext);
}

export function closeActiveQueryTab(): void {
  const { activeTabId, closeTab } = useQueryTabsStore.getState();
  if (activeTabId) closeTab(activeTabId);
}
