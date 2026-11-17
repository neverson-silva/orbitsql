import { useNavigate } from "@tanstack/react-router";
import type { MouseEvent } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { Icon } from "../../components/ui/icons";
import { useConnectionsStore } from "../../stores/connections.store";
import {
  type QueryTab,
  useQueryTabsStore,
} from "../../stores/query-tabs.store";
import {
  connectionProfileName,
  profileColorHex,
} from "../connections/profile-style";

export type { QueryTab } from "../../stores/query-tabs.store";
export {
  closeActiveQueryTab,
  openQueryTab,
  useQueryTabsStore,
} from "../../stores/query-tabs.store";

function QueryTabItem({ tab }: { tab: QueryTab }) {
  const navigate = useNavigate();
  const activeTabId = useQueryTabsStore((state) => state.activeTabId);
  const setActiveTab = useQueryTabsStore((state) => state.setActiveTab);
  const closeTab = useQueryTabsStore((state) => state.closeTab);
  const connection = useConnectionsStore((state) =>
    tab.connectionId ? state.connections[tab.connectionId] : undefined,
  );
  const active = activeTabId === tab.id;
  const dirty = tab.query.length > 0 && tab.query !== tab.lastExecutedQuery;
  const profile = connectionProfileName(connection?.detail);
  const profileColor = profileColorHex(connection?.profileColor);

  function selectTab() {
    setActiveTab(tab.id);
    void navigate({ to: "/query" });
  }

  function closeCurrentTab(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    closeTab(tab.id);
  }

  return (
    <div
      className={cn(
        "group flex h-full min-w-0 max-w-60 shrink-0 items-center overflow-hidden border border-b-2 transition-colors first:rounded-tl-sm last:rounded-tr-sm",
        "border-outline-variant/50",
        active
          ? "border-b-primary bg-selected-surface/50 text-on-surface"
          : "border-b-transparent bg-transparent text-on-surface-variant hover:bg-control-surface-hover",
      )}
      style={
        profile
          ? {
              backgroundImage: `linear-gradient(color-mix(in srgb, ${profileColor} 10%, transparent), color-mix(in srgb, ${profileColor} 10%, transparent))`,
              borderTopColor: `color-mix(in srgb, color-mix(in srgb, var(--ds-color-outline-variant) 50%, transparent) 65%, color-mix(in srgb, ${profileColor} 50%, transparent) 35%)`,
              borderInlineStartColor: `color-mix(in srgb, color-mix(in srgb, var(--ds-color-outline-variant) 50%, transparent) 65%, color-mix(in srgb, ${profileColor} 50%, transparent) 35%)`,
              borderInlineEndColor: `color-mix(in srgb, color-mix(in srgb, var(--ds-color-outline-variant) 50%, transparent) 65%, color-mix(in srgb, ${profileColor} 50%, transparent) 35%)`,
            }
          : undefined
      }
    >
      <button
        aria-pressed={active}
        className="flex min-w-0 flex-1 cursor-pointer items-center gap-1.5 overflow-hidden px-2.5 text-left font-mono text-label-caps outline-none focus-visible:ring-1 focus-visible:ring-ring"
        onClick={selectTab}
        type="button"
      >
        <Icon name="terminal" size={12} />
        <span className="truncate">{tab.title}</span>
        {dirty ? (
          <span className="shrink-0 text-primary" title="Unsaved changes">
            •
          </span>
        ) : null}
      </button>
      <Button
        aria-label={`Close ${tab.title}`}
        className="h-7 w-7 shrink-0 rounded-sm p-0 opacity-70 hover:opacity-100"
        onClick={closeCurrentTab}
        size="icon-sm"
        type="button"
        variant="ghost"
      >
        <Icon name="close" size={11} />
      </Button>
    </div>
  );
}

export function QueryTabs() {
  const tabs = useQueryTabsStore((state) => state.tabs);

  return (
    <nav
      aria-label="Open query scripts"
      className="flex h-full min-w-0 scroll-ps-0 items-stretch gap-0 overflow-x-auto ps-2"
    >
      {tabs.map((tab) => (
        <QueryTabItem key={tab.id} tab={tab} />
      ))}
    </nav>
  );
}
