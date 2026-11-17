import { beforeEach, describe, expect, it, vi } from "vitest";
import { refreshConnectionCatalog as refreshCatalogIpc } from "../features/connections/connection-ipc";
import { createCatalog } from "../features/sql-intelligence/tests/fixtures";
import { useCatalogStore } from "./catalog.store";

vi.mock("../features/connections/connection-ipc", () => ({
  refreshConnectionCatalog: vi.fn(),
}));

describe("catalog store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("keeps immutable indexed snapshots per connection", () => {
    const first = createCatalog().snapshot;
    const second = {
      ...first,
      id: "snapshot:second",
      connectionId: "connection:second",
      version: "catalog-v2",
    };
    useCatalogStore.getState().setSnapshot(first);
    const previousSnapshots = useCatalogStore.getState().snapshots;
    useCatalogStore.getState().setSnapshot(second);
    const state = useCatalogStore.getState();
    expect(previousSnapshots.get("connection:second")).toBeUndefined();
    expect(state.snapshots.get("connection:test")?.snapshot).toBe(first);
    expect(state.snapshots.get("connection:second")?.snapshot).toBe(second);
    expect(state.snapshot).toBe(second);
    expect(state.catalog.snapshot).toBe(second);
  });

  it("does not let an older refresh overwrite a newer refresh", async () => {
    const firstSnapshot = {
      ...createCatalog().snapshot,
      version: "catalog-v1",
    };
    const secondSnapshot = {
      ...firstSnapshot,
      id: "snapshot:latest",
      version: "catalog-v2",
    };
    let resolveFirst: (snapshot: typeof firstSnapshot) => void = () => {};
    const first = new Promise<typeof firstSnapshot>((resolve) => {
      resolveFirst = resolve;
    });
    vi.mocked(refreshCatalogIpc)
      .mockReturnValueOnce(first)
      .mockResolvedValueOnce(secondSnapshot);
    const staleRefresh = useCatalogStore
      .getState()
      .refreshCatalog("connection:test");
    await useCatalogStore.getState().refreshCatalog("connection:test");
    resolveFirst(firstSnapshot);
    await staleRefresh;
    expect(useCatalogStore.getState().snapshot.version).toBe("catalog-v2");
  });
});
