export const COMMAND_IDS = {
  fileNewQuery: "file.newQuery",
  fileNewConnection: "file.newConnection",
  fileOpenSqlFile: "file.openSqlFile",
  fileOpenRecent: "file.openRecent",
  fileSave: "file.save",
  fileSaveAs: "file.saveAs",
  fileCloseTab: "file.closeTab",
  fileCloseWindow: "file.closeWindow",
  windowClose: "window.close",
  editUndo: "edit.undo",
  editRedo: "edit.redo",
  editCut: "edit.cut",
  editCopy: "edit.copy",
  editPaste: "edit.paste",
  editSelectAll: "edit.selectAll",
  editFind: "edit.find",
  editFindNext: "edit.findNext",
  editFindPrevious: "edit.findPrevious",
  editReplace: "edit.replace",
  viewCommandPalette: "view.commandPalette",
  viewQuickOpen: "view.quickOpen",
  viewToggleSidebar: "view.toggleSidebar",
  viewOpenSettings: "view.openSettings",
  viewToggleResults: "view.toggleResults",
  viewToggleBottomPanel: "view.toggleBottomPanel",
  viewAppearanceLight: "view.appearance.light",
  viewAppearanceDark: "view.appearance.dark",
  viewAppearanceSystem: "view.appearance.system",
  viewZoomIn: "view.zoomIn",
  viewZoomOut: "view.zoomOut",
  viewResetZoom: "view.resetZoom",
  queryRun: "query.run",
  queryRunSelection: "query.runSelection",
  queryStop: "query.stop",
  queryExplain: "query.explain",
  queryExplainAnalyze: "query.explainAnalyze",
  queryFormat: "query.format",
  queryHistory: "query.history",
  databaseNewConnection: "database.newConnection",
  databaseConnect: "database.connect",
  databaseDisconnect: "database.disconnect",
  databaseReconnect: "database.reconnect",
  databaseActiveConnection: "database.activeConnection",
  databaseRefresh: "database.refresh",
  databaseRefreshSchema: "database.refreshSchema",
  databaseAutoCommit: "database.autoCommit",
  databaseCommit: "database.commit",
  databaseRollback: "database.rollback",
  windowMinimize: "window.minimize",
  windowToggleMaximize: "window.toggleMaximize",
  windowFullscreen: "window.fullscreen",
  windowBringAllToFront: "window.bringAllToFront",
  helpSearch: "help.search",
  helpDocumentation: "help.documentation",
  helpKeyboardShortcuts: "help.keyboardShortcuts",
  helpReportIssue: "help.reportIssue",
  helpCheckForUpdates: "help.checkForUpdates",
  helpAbout: "help.about",
} as const;

export type CommandId = (typeof COMMAND_IDS)[keyof typeof COMMAND_IDS];

export type MenuCommand = {
  kind: "command";
  id: CommandId;
  label: string;
  category: string;
  shortcut?: string;
  accelerator?: string;
};

export type MenuSeparator = {
  kind: "separator";
  id: string;
};

export type MenuSubmenu = {
  kind: "submenu";
  id: string;
  label: string;
  items: readonly MenuEntry[];
};

export type MenuEntry = MenuCommand | MenuSeparator | MenuSubmenu;

export type MenuGroup = {
  id: string;
  label: string;
  items: readonly MenuEntry[];
};

function command(
  id: CommandId,
  label: string,
  category: string,
  shortcut?: string,
  accelerator?: string,
): MenuCommand {
  return { kind: "command", id, label, category, shortcut, accelerator };
}

function separator(id: string): MenuSeparator {
  return { kind: "separator", id };
}

function submenu(
  id: string,
  label: string,
  items: readonly MenuEntry[],
): MenuSubmenu {
  return { kind: "submenu", id, label, items };
}

export const applicationMenu: readonly MenuGroup[] = [
  {
    id: "file",
    label: "File",
    items: [
      command(
        COMMAND_IDS.fileNewQuery,
        "New Query",
        "File",
        "⌘N",
        "CmdOrCtrl+N",
      ),
      command(
        COMMAND_IDS.fileNewConnection,
        "New Connection…",
        "File",
        "⌘⇧N",
        "CmdOrCtrl+Shift+N",
      ),
      separator("file-divider-open"),
      command(
        COMMAND_IDS.fileOpenSqlFile,
        "Open SQL File…",
        "File",
        "⌘O",
        "CmdOrCtrl+O",
      ),
      command(COMMAND_IDS.fileOpenRecent, "Open Recent", "File"),
      separator("file-divider-save"),
      command(COMMAND_IDS.fileSave, "Save", "File", "⌘S", "CmdOrCtrl+S"),
      command(
        COMMAND_IDS.fileSaveAs,
        "Save As…",
        "File",
        "⇧⌘S",
        "CmdOrCtrl+Shift+S",
      ),
      separator("file-divider-close"),
      command(
        COMMAND_IDS.fileCloseTab,
        "Close Tab",
        "File",
        "⌘W",
        "CmdOrCtrl+W",
      ),
      command(
        COMMAND_IDS.fileCloseWindow,
        "Close Window",
        "Window",
        "⇧⌘W",
        "CmdOrCtrl+Shift+W",
      ),
    ],
  },
  {
    id: "edit",
    label: "Edit",
    items: [
      command(COMMAND_IDS.editUndo, "Undo", "Edit", "⌘Z", "CmdOrCtrl+Z"),
      command(COMMAND_IDS.editRedo, "Redo", "Edit", "⇧⌘Z", "CmdOrCtrl+Shift+Z"),
      separator("edit-divider-clipboard"),
      command(COMMAND_IDS.editCut, "Cut", "Edit", "⌘X", "CmdOrCtrl+X"),
      command(COMMAND_IDS.editCopy, "Copy", "Edit", "⌘C", "CmdOrCtrl+C"),
      command(COMMAND_IDS.editPaste, "Paste", "Edit", "⌘V", "CmdOrCtrl+V"),
      command(
        COMMAND_IDS.editSelectAll,
        "Select All",
        "Edit",
        "⌘A",
        "CmdOrCtrl+A",
      ),
      separator("edit-divider-search"),
      command(COMMAND_IDS.editFind, "Find…", "Edit", "⌘F", "CmdOrCtrl+F"),
      command(
        COMMAND_IDS.editFindNext,
        "Find Next",
        "Edit",
        "⌘G",
        "CmdOrCtrl+G",
      ),
      command(
        COMMAND_IDS.editFindPrevious,
        "Find Previous",
        "Edit",
        "⇧⌘G",
        "CmdOrCtrl+Shift+G",
      ),
      command(
        COMMAND_IDS.editReplace,
        "Replace…",
        "Edit",
        "⌥⌘F",
        "Alt+CmdOrCtrl+F",
      ),
    ],
  },
  {
    id: "view",
    label: "View",
    items: [
      command(
        COMMAND_IDS.viewCommandPalette,
        "Command Palette…",
        "View",
        "⇧⌘P",
        "CmdOrCtrl+Shift+P",
      ),
      command(
        COMMAND_IDS.viewQuickOpen,
        "Quick Open…",
        "View",
        "⌘P",
        "CmdOrCtrl+P",
      ),
      separator("view-divider-layout"),
      command(
        COMMAND_IDS.viewToggleSidebar,
        "Toggle Sidebar",
        "View",
        "⌘B",
        "CmdOrCtrl+B",
      ),
      command(COMMAND_IDS.viewToggleResults, "Toggle Results", "View"),
      command(
        COMMAND_IDS.viewToggleBottomPanel,
        "Toggle Bottom Panel",
        "View",
        "⌘J",
        "CmdOrCtrl+J",
      ),
      command(
        COMMAND_IDS.viewOpenSettings,
        "Application Settings",
        "View",
        "⌘,",
        "CmdOrCtrl+,",
      ),
      separator("view-divider-appearance"),
      submenu("view-appearance", "Appearance", [
        command(COMMAND_IDS.viewAppearanceLight, "Light", "View / Appearance"),
        command(COMMAND_IDS.viewAppearanceDark, "Dark", "View / Appearance"),
        command(
          COMMAND_IDS.viewAppearanceSystem,
          "System",
          "View / Appearance",
        ),
      ]),
      separator("view-divider-zoom"),
      command(COMMAND_IDS.viewZoomIn, "Zoom In", "View", "⌘+", "CmdOrCtrl+="),
      command(COMMAND_IDS.viewZoomOut, "Zoom Out", "View", "⌘−", "CmdOrCtrl+-"),
      command(
        COMMAND_IDS.viewResetZoom,
        "Reset Zoom",
        "View",
        "⌘0",
        "CmdOrCtrl+0",
      ),
      command(
        COMMAND_IDS.windowFullscreen,
        "Enter Full Screen",
        "Window",
        "⌃⌘F",
        "Ctrl+Cmd+F",
      ),
    ],
  },
  {
    id: "query",
    label: "Query",
    items: [
      command(
        COMMAND_IDS.queryRun,
        "Run Query",
        "Query",
        "⌘↵",
        "CmdOrCtrl+Enter",
      ),
      command(COMMAND_IDS.queryRunSelection, "Run Selected Query", "Query"),
      command(COMMAND_IDS.queryStop, "Stop Query", "Query"),
      separator("query-divider-explain"),
      command(COMMAND_IDS.queryExplain, "Explain", "Query"),
      command(COMMAND_IDS.queryExplainAnalyze, "Explain Analyze", "Query"),
      separator("query-divider-format"),
      command(COMMAND_IDS.queryFormat, "Format SQL", "Query"),
      command(COMMAND_IDS.queryHistory, "Query History…", "Query"),
    ],
  },
  {
    id: "database",
    label: "Database",
    items: [
      command(
        COMMAND_IDS.databaseNewConnection,
        "New Connection…",
        "Database",
        "⌘⇧N",
        "CmdOrCtrl+Shift+N",
      ),
      separator("database-divider-session"),
      command(COMMAND_IDS.databaseConnect, "Connect", "Database"),
      command(COMMAND_IDS.databaseDisconnect, "Disconnect", "Database"),
      command(COMMAND_IDS.databaseReconnect, "Reconnect", "Database"),
      separator("database-divider-active"),
      command(
        COMMAND_IDS.databaseActiveConnection,
        "Active Connection",
        "Database",
      ),
      separator("database-divider-refresh"),
      command(COMMAND_IDS.databaseRefresh, "Refresh", "Database"),
      command(COMMAND_IDS.databaseRefreshSchema, "Refresh Schema", "Database"),
      separator("database-divider-transaction"),
      submenu("database-transaction", "Transaction", [
        command(
          COMMAND_IDS.databaseAutoCommit,
          "Auto-Commit",
          "Database / Transaction",
        ),
        command(COMMAND_IDS.databaseCommit, "Commit", "Database / Transaction"),
        command(
          COMMAND_IDS.databaseRollback,
          "Rollback",
          "Database / Transaction",
        ),
      ]),
    ],
  },
  {
    id: "window",
    label: "Window",
    items: [
      command(
        COMMAND_IDS.windowMinimize,
        "Minimize",
        "Window",
        "⌘M",
        "CmdOrCtrl+M",
      ),
      command(COMMAND_IDS.windowToggleMaximize, "Zoom", "Window"),
      command(
        COMMAND_IDS.windowFullscreen,
        "Enter Full Screen",
        "Window",
        "⌃⌘F",
        "Ctrl+Cmd+F",
      ),
      separator("window-divider-front"),
      command(
        COMMAND_IDS.windowBringAllToFront,
        "Bring All to Front",
        "Window",
      ),
      separator("window-divider-close"),
      command(COMMAND_IDS.windowClose, "Close Window", "Window"),
    ],
  },
  {
    id: "help",
    label: "Help",
    items: [
      command(COMMAND_IDS.helpSearch, "Search", "Help"),
      separator("help-divider-docs"),
      command(COMMAND_IDS.helpDocumentation, "Orbit SQL Documentation", "Help"),
      command(COMMAND_IDS.helpKeyboardShortcuts, "Keyboard Shortcuts", "Help"),
      separator("help-divider-support"),
      command(COMMAND_IDS.helpReportIssue, "Report an Issue…", "Help"),
      command(COMMAND_IDS.helpCheckForUpdates, "Check for Updates…", "Help"),
      separator("help-divider-about"),
      command(COMMAND_IDS.helpAbout, "About Orbit SQL", "Help"),
    ],
  },
] as const;

export function commandForKeyboardEvent(
  event: KeyboardEvent,
): CommandId | undefined {
  const commands = flattenMenuCommands();
  return commands.find((entry) => {
    if (!entry.accelerator) return false;
    const tokens = entry.accelerator.split("+");
    const key = tokens[tokens.length - 1];
    if (!key || event.key.toLowerCase() !== key.toLowerCase()) return false;
    if (event.shiftKey !== tokens.includes("Shift")) return false;
    if (event.altKey !== tokens.includes("Alt")) return false;
    const requiresBothModifiers =
      tokens.includes("Ctrl") && tokens.includes("Cmd");
    const hasModifier = event.metaKey || event.ctrlKey;
    if (requiresBothModifiers) return event.metaKey && event.ctrlKey;
    if (tokens.includes("CmdOrCtrl")) return hasModifier;
    return true;
  })?.id;
}

export function flattenMenuCommands(
  entries: readonly MenuEntry[] = applicationMenu.flatMap(
    (group) => group.items,
  ),
): MenuCommand[] {
  return entries.flatMap((entry) => {
    if (entry.kind === "command") return [entry];
    if (entry.kind === "submenu") return flattenMenuCommands(entry.items);
    return [];
  });
}
