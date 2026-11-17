import type { KeyboardEvent, PointerEvent as ReactPointerEvent } from "react";
import { ExecutionContextSidebar } from "../../features/query/execution-context-sidebar";
import { SIDEBAR_WIDTH, useSettingsStore } from "../../stores/settings.store";
import { cn } from "../ui/ui";
import { ConnectionExplorer } from "./connection-explorer";
import { SidebarHeader } from "./header";

export function SidebarShell({
  onAddConnection,
  isResizing,
  sidebarWidth,
  startResizing,
  stopResizing,
  resizeWithKeyboard,
}: {
  onAddConnection: () => void;
  isResizing: boolean;
  sidebarWidth: number;
  startResizing: (event: ReactPointerEvent<HTMLElement>) => void;
  stopResizing: () => void;
  resizeWithKeyboard: (event: KeyboardEvent<HTMLElement>) => void;
}) {
  const sidebarOpen = useSettingsStore((state) => state.sidebarOpen);

  return (
    <aside
      className="relative min-h-0 min-w-0 overflow-visible"
      data-slot="sidebar-shell"
    >
      <section
        className="app-sidebar-surface grid h-full min-h-0 min-w-0 grid-rows-[var(--ds-workbench-header-height)_auto_auto_minmax(0,1fr)]"
        data-slot="sidebar-explorer"
      >
        {sidebarOpen ? (
          <SidebarHeader onAddConnection={onAddConnection} />
        ) : null}
        {sidebarOpen ? (
          <div className="min-w-0  px-2 pb-2 pt-1">
            <ExecutionContextSidebar />
          </div>
        ) : null}
        <span className="mx-2 mt-3 border-b border-outline-variant/50" />
        <div className="relative h-full min-h-0 min-w-0 overflow-hidden">
          {sidebarOpen ? <ConnectionExplorer /> : null}
        </div>
      </section>
      {sidebarOpen ? (
        <button
          aria-label="Resize navigation sidebar"
          aria-orientation="vertical"
          aria-valuemax={SIDEBAR_WIDTH.max}
          aria-valuemin={SIDEBAR_WIDTH.min}
          aria-valuenow={sidebarWidth}
          className={cn("sidebar-resize-handle", isResizing && "is-resizing")}
          onKeyDown={resizeWithKeyboard}
          onPointerCancel={stopResizing}
          onPointerDown={startResizing}
          onPointerUp={stopResizing}
          role="slider"
          tabIndex={0}
          type="button"
        />
      ) : null}
    </aside>
  );
}
