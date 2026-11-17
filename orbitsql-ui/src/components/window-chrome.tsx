import { useNavigate } from "@tanstack/react-router";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  type CSSProperties,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { APP_NAME, APP_VERSION_LABEL } from "../app-meta";
import {
  applicationMenu,
  COMMAND_IDS,
  type CommandId,
  commandForKeyboardEvent,
  flattenMenuCommands,
  type MenuCommand,
  type MenuEntry,
  type MenuGroup,
} from "../commands/command-model";
import {
  executeCommand,
  getCommandState,
  getCommandStateSnapshot,
  registerCommandHandlers,
  useCommandRegistryVersion,
} from "../commands/command-registry";
import { themeRegistry } from "../design-system";
import { useConnectionsStore } from "../stores/connections.store";
import { closeActiveQueryTab, openQueryTab } from "../stores/query-tabs.store";
import { useSettingsStore } from "../stores/settings.store";
import { MACOS_TRAFFIC_LIGHT_SAFE_AREA } from "../window/geometry";
import { getPlatform, isTauriRuntime } from "../window/platform";
import { windowController } from "../window/window-controller";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "./ui/command";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";
import { Icon } from "./ui/icons";
import { IconButton, StatusPill } from "./ui/ui";

function presentShortcut(command: MenuCommand): string | undefined {
  if (getPlatform() === "macos") return command.shortcut;
  return command.accelerator
    ?.replace("CmdOrCtrl", "Ctrl")
    .replace("Ctrl+Cmd+", "Ctrl+");
}

function runEditingCommand(command: CommandId): void {
  const actions: Partial<Record<CommandId, string>> = {
    [COMMAND_IDS.editUndo]: "undo",
    [COMMAND_IDS.editRedo]: "redo",
    [COMMAND_IDS.editCut]: "cut",
    [COMMAND_IDS.editCopy]: "copy",
    [COMMAND_IDS.editPaste]: "paste",
    [COMMAND_IDS.editSelectAll]: "selectAll",
  };
  const action = actions[command];
  if (action) document.execCommand(action);
}

function applyAppearance(mode: "light" | "dark" | "system"): void {
  const resolvedMode =
    mode === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : mode;
  const theme =
    themeRegistry.getAll().find((candidate) => candidate.id === resolvedMode) ??
    themeRegistry.getAll().find((candidate) => candidate.type === resolvedMode);
  if (theme) useSettingsStore.getState().setThemeId(theme.id);
}

function MenuEntries({
  entries,
  closeMenu,
}: {
  entries: readonly MenuEntry[];
  closeMenu: () => void;
}) {
  return (
    <>
      {entries.map((entry) => {
        if (entry.kind === "separator") {
          return <DropdownMenuSeparator key={entry.id} />;
        }

        if (entry.kind === "submenu") {
          return (
            <DropdownMenuSub key={entry.id}>
              <DropdownMenuSubTrigger>{entry.label}</DropdownMenuSubTrigger>
              <DropdownMenuSubContent>
                <MenuEntries entries={entry.items} closeMenu={closeMenu} />
              </DropdownMenuSubContent>
            </DropdownMenuSub>
          );
        }

        const state = getCommandState(entry.id);
        if (!state.visible) return null;
        const content = (
          <>
            <span>{entry.label}</span>
            {presentShortcut(entry) ? (
              <DropdownMenuShortcut>
                {presentShortcut(entry)}
              </DropdownMenuShortcut>
            ) : null}
          </>
        );
        if (state.checked === undefined) {
          return (
            <DropdownMenuItem
              disabled={!state.enabled}
              key={entry.id}
              onClick={() => {
                closeMenu();
                void executeCommand(entry.id).catch(() => undefined);
              }}
            >
              {content}
            </DropdownMenuItem>
          );
        }
        return (
          <DropdownMenuCheckboxItem
            checked={state.checked}
            disabled={!state.enabled}
            key={entry.id}
            onClick={() => {
              closeMenu();
              void executeCommand(entry.id).catch(() => undefined);
            }}
          >
            {content}
          </DropdownMenuCheckboxItem>
        );
      })}
    </>
  );
}

function ApplicationMenuBar() {
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  useCommandRegistryVersion();

  const renderGroup = useCallback(
    (group: MenuGroup) => (
      <DropdownMenu
        key={group.id}
        modal={false}
        onOpenChange={(open) => setOpenMenu(open ? group.id : null)}
        open={openMenu === group.id}
      >
        <DropdownMenuTrigger
          aria-label={`${group.label} menu`}
          className="h-7 rounded-sm px-2 text-xs font-medium text-on-surface-variant outline-none transition-colors hover:bg-control-surface-hover hover:text-on-surface focus-visible:bg-control-surface-hover data-[popup-open]:bg-control-surface-active data-[popup-open]:text-on-surface"
          onPointerEnter={() => {
            if (openMenu) setOpenMenu(group.id);
          }}
        >
          {group.label}
        </DropdownMenuTrigger>
        <DropdownMenuContent
          align="start"
          className="min-w-52 p-1"
          sideOffset={2}
        >
          <MenuEntries
            closeMenu={() => setOpenMenu(null)}
            entries={group.items}
          />
        </DropdownMenuContent>
      </DropdownMenu>
    ),
    [openMenu],
  );

  return (
    <div
      aria-label="Application menu"
      className="flex h-full shrink-0 items-center gap-0.5 px-2"
      role="menubar"
    >
      {applicationMenu.map(renderGroup)}
    </div>
  );
}

function WindowControls({ maximized }: { maximized: boolean }) {
  return (
    <div className="flex h-full shrink-0 items-center">
      <button
        aria-label="Minimize window"
        className="flex h-full w-10 cursor-pointer items-center justify-center text-on-surface-variant transition-colors hover:bg-control-surface-hover hover:text-on-surface focus-visible:bg-control-surface-hover"
        onClick={() => void windowController.minimize()}
        onDoubleClick={(event) => event.stopPropagation()}
        onPointerDown={(event) => event.stopPropagation()}
        type="button"
      >
        <Icon name="minus" size={14} />
      </button>
      <button
        aria-label={maximized ? "Restore window" : "Maximize window"}
        className="flex h-full w-10 cursor-pointer items-center justify-center text-on-surface-variant transition-colors hover:bg-control-surface-hover hover:text-on-surface focus-visible:bg-control-surface-hover"
        onClick={() => void windowController.toggleMaximize()}
        onDoubleClick={(event) => event.stopPropagation()}
        onPointerDown={(event) => event.stopPropagation()}
        type="button"
      >
        <Icon name={maximized ? "restore" : "maximize"} size={14} />
      </button>
      <button
        aria-label="Close window"
        className="flex h-full w-10 cursor-pointer items-center justify-center text-on-surface-variant transition-colors hover:bg-error hover:text-on-error focus-visible:bg-error focus-visible:text-on-error"
        onClick={() => void windowController.close()}
        onDoubleClick={(event) => event.stopPropagation()}
        onPointerDown={(event) => event.stopPropagation()}
        type="button"
      >
        <Icon name="close" size={16} />
      </button>
    </div>
  );
}

function WindowActions({ onOpenSettings }: { onOpenSettings: () => void }) {
  return (
    <div className="flex h-full shrink-0 items-center gap-1.5 pe-3 ps-1">
      <IconButton
        label="Quick Open"
        name="search"
        onClick={() =>
          void executeCommand(COMMAND_IDS.viewQuickOpen).catch(() => undefined)
        }
        size="md"
      />
      <StatusPill
        className="h-6 shrink-0 gap-1 px-2 text-[10px]"
        icon="circle"
        tone="success"
      >
        12ms
      </StatusPill>
      <IconButton label="Notifications" name="bell" size="md" />
      <IconButton
        label="Open application settings"
        name="settings"
        onClick={onOpenSettings}
        size="md"
      />
    </div>
  );
}

function CommandPalette({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  useCommandRegistryVersion();
  const commands = Array.from(
    new Map(
      flattenMenuCommands().map((command) => [command.id, command]),
    ).values(),
  );

  return (
    <CommandDialog
      className="sm:max-w-xl"
      description="Search and run an Orbit SQL command."
      onOpenChange={onOpenChange}
      open={open}
      title="Orbit SQL command palette"
    >
      <Command>
        <CommandInput placeholder="Search commands..." />
        <CommandList>
          <CommandEmpty>No matching commands.</CommandEmpty>
          <CommandGroup>
            <div className="px-2 py-2 text-xs text-muted-foreground">
              Commands
            </div>
            {commands.map((command) => {
              const state = getCommandState(command.id);
              if (!state.visible) return null;
              return (
                <CommandItem
                  disabled={!state.enabled}
                  key={command.id}
                  onClick={() => {
                    onOpenChange(false);
                    void executeCommand(command.id).catch(() => undefined);
                  }}
                  value={`${command.label} ${command.id}`}
                >
                  <span className="text-muted-foreground">
                    {command.category}:
                  </span>
                  <span>{command.label}</span>
                  {presentShortcut(command) ? (
                    <CommandShortcut>
                      {presentShortcut(command)}
                    </CommandShortcut>
                  ) : null}
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
      </Command>
    </CommandDialog>
  );
}

export function WindowChrome() {
  const navigate = useNavigate();
  const sidebarOpen = useSettingsStore((state) => state.sidebarOpen);
  const themeId = useSettingsStore((state) => state.themeId);
  const toggleSidebar = useSettingsStore((state) => state.toggleSidebar);
  const [maximized, setMaximized] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const dragRegionRef = useRef<HTMLDivElement>(null);
  const dragTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const platform = getPlatform();
  const commandRegistryVersion = useCommandRegistryVersion();

  useEffect(() => {
    const unregister = registerCommandHandlers(
      {
        [COMMAND_IDS.fileNewQuery]: () => {
          openQueryTab(useConnectionsStore.getState().activeConnectionId);
          return navigate({ to: "/query" });
        },
        [COMMAND_IDS.fileCloseTab]: () => closeActiveQueryTab(),
        [COMMAND_IDS.fileNewConnection]: () =>
          navigate({ to: "/connections/new" }),
        [COMMAND_IDS.fileCloseWindow]: () => windowController.close(),
        [COMMAND_IDS.viewCommandPalette]: () => setPaletteOpen(true),
        [COMMAND_IDS.editUndo]: () => runEditingCommand(COMMAND_IDS.editUndo),
        [COMMAND_IDS.editRedo]: () => runEditingCommand(COMMAND_IDS.editRedo),
        [COMMAND_IDS.editCut]: () => runEditingCommand(COMMAND_IDS.editCut),
        [COMMAND_IDS.editCopy]: () => runEditingCommand(COMMAND_IDS.editCopy),
        [COMMAND_IDS.editPaste]: () => runEditingCommand(COMMAND_IDS.editPaste),
        [COMMAND_IDS.editSelectAll]: () =>
          runEditingCommand(COMMAND_IDS.editSelectAll),
        [COMMAND_IDS.viewQuickOpen]: () => setPaletteOpen(true),
        [COMMAND_IDS.viewToggleSidebar]: toggleSidebar,
        [COMMAND_IDS.viewOpenSettings]: () => navigate({ to: "/settings" }),
        [COMMAND_IDS.viewAppearanceLight]: () => applyAppearance("light"),
        [COMMAND_IDS.viewAppearanceDark]: () => applyAppearance("dark"),
        [COMMAND_IDS.viewAppearanceSystem]: () => applyAppearance("system"),
        [COMMAND_IDS.viewZoomIn]: () => useSettingsStore.getState().zoomIn(),
        [COMMAND_IDS.viewZoomOut]: () => useSettingsStore.getState().zoomOut(),
        [COMMAND_IDS.viewResetZoom]: () =>
          useSettingsStore.getState().resetZoom(),
        [COMMAND_IDS.databaseNewConnection]: () =>
          navigate({ to: "/connections/new" }),
        [COMMAND_IDS.windowMinimize]: () => windowController.minimize(),
        [COMMAND_IDS.windowToggleMaximize]: () =>
          windowController.toggleMaximize(),
        [COMMAND_IDS.windowFullscreen]: () =>
          windowController.toggleFullscreen(),
        [COMMAND_IDS.windowClose]: () => windowController.close(),
        [COMMAND_IDS.helpAbout]: () => setAboutOpen(true),
      },
      {
        [COMMAND_IDS.viewToggleSidebar]: { checked: sidebarOpen },
        [COMMAND_IDS.viewAppearanceLight]: {
          checked: themeRegistry.get(themeId)?.type === "light",
        },
        [COMMAND_IDS.viewAppearanceDark]: {
          checked: themeRegistry.get(themeId)?.type === "dark",
        },
      },
    );
    return unregister;
  }, [navigate, sidebarOpen, themeId, toggleSidebar]);

  useEffect(() => {
    if (!isTauriRuntime() || platform !== "macos") return;
    void invoke("sync_menu_state", {
      states: getCommandStateSnapshot(),
      version: commandRegistryVersion,
    }).catch(() => undefined);
  }, [commandRegistryVersion, platform]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    if (isTauriRuntime() && platform === "macos") {
      void listen<string>("application-command", (event) => {
        void executeCommand(event.payload as CommandId).catch(() => undefined);
      }).then((cleanup) => {
        if (active) unlisten = cleanup;
        else cleanup();
      });
    }

    return () => {
      active = false;
      unlisten?.();
    };
  }, [platform]);

  useEffect(() => {
    let active = true;
    let cleanup: (() => void) | undefined;
    void windowController
      .observeMaximized((value) => {
        if (active) setMaximized(value);
      })
      .then((nextCleanup) => {
        if (active) cleanup = nextCleanup;
        else nextCleanup();
      });
    return () => {
      active = false;
      cleanup?.();
    };
  }, []);

  useEffect(() => {
    function onZoomKeyDown(event: KeyboardEvent): void {
      if (!(event.metaKey || event.ctrlKey) || event.altKey) return;

      let command: CommandId | undefined;
      if (
        event.key === "+" ||
        event.key === "=" ||
        event.code === "NumpadAdd"
      ) {
        command = COMMAND_IDS.viewZoomIn;
      } else if (event.key === "-" || event.code === "NumpadSubtract") {
        command = COMMAND_IDS.viewZoomOut;
      } else if (event.key === "0" || event.code === "Numpad0") {
        command = COMMAND_IDS.viewResetZoom;
      }

      if (!command) return;
      event.preventDefault();
      event.stopPropagation();
      void executeCommand(command).catch(() => undefined);
    }

    window.addEventListener("keydown", onZoomKeyDown, true);
    return () => window.removeEventListener("keydown", onZoomKeyDown, true);
  }, []);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      const command = commandForKeyboardEvent(event);
      if (!command || !getCommandState(command).enabled) return;
      if (command.startsWith("edit.")) return;
      if (
        (command === COMMAND_IDS.windowClose ||
          command === COMMAND_IDS.fileCloseWindow) &&
        !isTauriRuntime()
      ) {
        return;
      }
      event.preventDefault();
      void executeCommand(command).catch(() => undefined);
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    const dragRegion = dragRegionRef.current;
    if (!dragRegion) return;

    function handleDragMouseDown(event: globalThis.MouseEvent): void {
      console.log("[window-chrome] titlebar:mousedown", {
        buttons: event.buttons,
        detail: event.detail,
        target: event.target,
      });
      if (event.buttons !== 1) return;
      if (event.detail !== 1) return;
      if (dragTimerRef.current) clearTimeout(dragTimerRef.current);
      dragTimerRef.current = setTimeout(() => {
        dragTimerRef.current = null;
        console.log("[window-chrome] titlebar:drag-start");
        void windowController.startDragging().catch((error) => {
          console.error("[window-chrome] titlebar:drag-error", error);
        });
      }, 160);
    }

    function handleDragDoubleClick(): void {
      console.log("[window-chrome] titlebar:dblclick");
      if (dragTimerRef.current) {
        clearTimeout(dragTimerRef.current);
        dragTimerRef.current = null;
      }
      void windowController.toggleMaximize().catch((error) => {
        console.error("[window-chrome] titlebar:maximize-error", error);
      });
    }

    dragRegion.addEventListener("mousedown", handleDragMouseDown);
    dragRegion.addEventListener("dblclick", handleDragDoubleClick);
    return () => {
      dragRegion.removeEventListener("mousedown", handleDragMouseDown);
      dragRegion.removeEventListener("dblclick", handleDragDoubleClick);
      if (dragTimerRef.current) clearTimeout(dragTimerRef.current);
    };
  }, []);

  return (
    <>
      <div
        aria-label="Window chrome"
        className="flex h-full min-w-0 items-center bg-app-chrome text-on-surface"
        data-platform={platform}
        role="toolbar"
        style={
          {
            "--ds-traffic-light-safe-area": MACOS_TRAFFIC_LIGHT_SAFE_AREA,
          } as CSSProperties
        }
      >
        {platform === "macos" ? (
          <>
            <div
              aria-hidden="true"
              className="w-(--ds-traffic-light-safe-area) shrink-0"
            />
            <div
              aria-hidden="true"
              className="h-full min-w-0 flex-1 select-none bg-transparent"
              ref={dragRegionRef}
            />
            <WindowActions
              onOpenSettings={() => navigate({ to: "/settings" })}
            />
          </>
        ) : (
          <>
            <ApplicationMenuBar />
            <div
              aria-hidden="true"
              className="h-full min-w-0 flex-1 select-none bg-transparent"
              ref={dragRegionRef}
            />
            <WindowActions
              onOpenSettings={() => navigate({ to: "/settings" })}
            />
            <WindowControls maximized={maximized} />
          </>
        )}
      </div>
      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />
      <Dialog onOpenChange={setAboutOpen} open={aboutOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>About {APP_NAME}</DialogTitle>
            <DialogDescription>
              A native-feeling database workspace for querying, exploring, and
              managing connections.
            </DialogDescription>
          </DialogHeader>
          <div className="rounded-md border border-surface-divider bg-surface-subtle px-3 py-2 font-mono text-label-caps text-on-surface-variant">
            {APP_NAME} {APP_VERSION_LABEL}
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
