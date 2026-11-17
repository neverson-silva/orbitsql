import { beforeEach, describe, expect, it } from "vitest";
import { useQueryTabsStore } from "./query-tabs.store";

describe("query tabs", () => {
  beforeEach(() => {
    useQueryTabsStore.setState({ tabs: [], activeTabId: null });
  });

  it("opens tabs with independent connection and editor state", () => {
    const firstTabId = useQueryTabsStore.getState().openTab("postgresql-1");
    const secondTabId = useQueryTabsStore.getState().openTab("mssql-1");

    useQueryTabsStore.getState().updateTab(firstTabId, {
      query: "select 1",
      resultTab: "Result View",
    });
    useQueryTabsStore.getState().updateTab(secondTabId, {
      query: "select 2",
      resultTab: "Result View",
    });

    const state = useQueryTabsStore.getState();
    expect(state.tabs).toHaveLength(2);
    expect(state.activeTabId).toBe(secondTabId);
    expect(state.tabs[0]).toMatchObject({
      id: firstTabId,
      connectionId: "postgresql-1",
      query: "select 1",
    });
    expect(state.tabs[1]).toMatchObject({
      id: secondTabId,
      connectionId: "mssql-1",
      query: "select 2",
    });
  });

  it("selects a tab and keeps its state after switching", () => {
    const firstTabId = useQueryTabsStore.getState().openTab("postgresql-1");
    const secondTabId = useQueryTabsStore.getState().openTab("mssql-1");

    useQueryTabsStore.getState().updateTab(secondTabId, {
      query: "select top 10 * from users",
      errorMessage: "temporary error",
    });
    useQueryTabsStore.getState().setActiveTab(firstTabId);
    useQueryTabsStore.getState().setActiveTab(secondTabId);

    expect(useQueryTabsStore.getState()).toMatchObject({
      activeTabId: secondTabId,
      tabs: [
        { id: firstTabId, query: "", connectionId: "postgresql-1" },
        {
          id: secondTabId,
          query: "select top 10 * from users",
          connectionId: "mssql-1",
          errorMessage: "temporary error",
        },
      ],
    });
  });

  it("keeps execution context scoped to each query tab", () => {
    const firstTabId = useQueryTabsStore.getState().openTab("production");
    const secondTabId = useQueryTabsStore.getState().openTab("local");

    useQueryTabsStore.getState().setExecutionContext(firstTabId, {
      connectionId: "production",
      dimensions: { database: "customer-prod" },
      connectionState: "ready",
    });
    useQueryTabsStore.getState().setExecutionContext(secondTabId, {
      connectionId: "local",
      dimensions: { database: "postgres" },
      connectionState: "ready",
    });

    useQueryTabsStore.getState().setActiveTab(firstTabId);
    expect(useQueryTabsStore.getState().tabs[0]?.executionContext).toEqual({
      connectionId: "production",
      dimensions: { database: "customer-prod" },
      connectionState: "ready",
    });
    useQueryTabsStore.getState().setActiveTab(secondTabId);
    expect(useQueryTabsStore.getState().tabs[1]?.executionContext).toEqual({
      connectionId: "local",
      dimensions: { database: "postgres" },
      connectionState: "ready",
    });
  });

  it("closes the active tab and chooses the nearest remaining tab", () => {
    const firstTabId = useQueryTabsStore.getState().openTab("postgresql-1");
    const secondTabId = useQueryTabsStore.getState().openTab("mssql-1");
    const thirdTabId = useQueryTabsStore.getState().openTab("postgresql-2");

    useQueryTabsStore.getState().setActiveTab(secondTabId);
    useQueryTabsStore.getState().closeTab(secondTabId);

    expect(useQueryTabsStore.getState()).toMatchObject({
      activeTabId: thirdTabId,
      tabs: [{ id: firstTabId }, { id: thirdTabId }],
    });

    useQueryTabsStore.getState().closeTab(thirdTabId);
    useQueryTabsStore.getState().closeTab(firstTabId);

    expect(useQueryTabsStore.getState()).toMatchObject({
      activeTabId: null,
      tabs: [],
    });
  });
});
