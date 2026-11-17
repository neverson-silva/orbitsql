use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItem, MenuItemBuilder,
    MenuItemKind, PredefinedMenuItem, Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Runtime};

pub fn set_command_state<R: Runtime>(
    menu: &Menu<R>,
    id: &str,
    enabled: bool,
    checked: Option<bool>,
) -> tauri::Result<()> {
    for item in menu.items()? {
        if item.id().as_ref() == id {
            apply_menu_item_state(&item, enabled, checked)?;
        }
        if let Some(submenu) = item.as_submenu() {
            set_submenu_command_state(submenu, id, enabled, checked)?;
        }
    }
    Ok(())
}

fn set_submenu_command_state<R: Runtime>(
    submenu: &Submenu<R>,
    id: &str,
    enabled: bool,
    checked: Option<bool>,
) -> tauri::Result<()> {
    for item in submenu.items()? {
        if item.id().as_ref() == id {
            apply_menu_item_state(&item, enabled, checked)?;
        }
        if let Some(child) = item.as_submenu() {
            set_submenu_command_state(child, id, enabled, checked)?;
        }
    }
    Ok(())
}

fn apply_menu_item_state<R: Runtime>(
    item: &MenuItemKind<R>,
    enabled: bool,
    checked: Option<bool>,
) -> tauri::Result<()> {
    match item {
        MenuItemKind::MenuItem(item) => item.set_enabled(enabled)?,
        MenuItemKind::Check(item) => {
            item.set_enabled(enabled)?;
            if let Some(checked) = checked {
                item.set_checked(checked)?;
            }
        }
        MenuItemKind::Icon(item) => item.set_enabled(enabled)?,
        MenuItemKind::Submenu(item) => item.set_enabled(enabled)?,
        MenuItemKind::Predefined(_) => {}
    }
    Ok(())
}

fn command<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    label: &str,
    accelerator: Option<&str>,
) -> tauri::Result<MenuItem<R>> {
    let builder = MenuItemBuilder::with_id(id, label).enabled(false);
    let builder = if let Some(accelerator) = accelerator {
        builder.accelerator(accelerator)
    } else {
        builder
    };
    builder.build(app)
}

fn check_command<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    label: &str,
    accelerator: Option<&str>,
) -> tauri::Result<CheckMenuItem<R>> {
    let builder = CheckMenuItemBuilder::with_id(id, label)
        .enabled(false)
        .checked(false);
    let builder = if let Some(accelerator) = accelerator {
        builder.accelerator(accelerator)
    } else {
        builder
    };
    builder.build(app)
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let settings = command(app, "view.openSettings", "Settings…", Some("CmdOrCtrl+,"))?;
    let app_menu = SubmenuBuilder::new(app, "Orbit SQL")
        .about(None)
        .separator()
        .item(&settings)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let new_query = command(app, "file.newQuery", "New Query", Some("CmdOrCtrl+N"))?;
    let new_connection = command(
        app,
        "file.newConnection",
        "New Connection…",
        Some("CmdOrCtrl+Shift+N"),
    )?;
    let open_sql_file = command(
        app,
        "file.openSqlFile",
        "Open SQL File…",
        Some("CmdOrCtrl+O"),
    )?;
    let open_recent = command(app, "file.openRecent", "Open Recent", None)?;
    let save = command(app, "file.save", "Save", Some("CmdOrCtrl+S"))?;
    let save_as = command(app, "file.saveAs", "Save As…", Some("CmdOrCtrl+Shift+S"))?;
    let close_tab = command(app, "file.closeTab", "Close Tab", Some("CmdOrCtrl+W"))?;
    let close_window = command(
        app,
        "file.closeWindow",
        "Close Window",
        Some("CmdOrCtrl+Shift+W"),
    )?;
    let file_menu = SubmenuBuilder::with_id(app, "file", "File")
        .item(&new_query)
        .item(&new_connection)
        .separator()
        .item(&open_sql_file)
        .item(&open_recent)
        .separator()
        .item(&save)
        .item(&save_as)
        .separator()
        .item(&close_tab)
        .item(&close_window)
        .build()?;

    let find = command(app, "edit.find", "Find…", Some("CmdOrCtrl+F"))?;
    let find_next = command(app, "edit.findNext", "Find Next", Some("CmdOrCtrl+G"))?;
    let find_previous = command(
        app,
        "edit.findPrevious",
        "Find Previous",
        Some("CmdOrCtrl+Shift+G"),
    )?;
    let replace = command(app, "edit.replace", "Replace…", Some("Alt+CmdOrCtrl+F"))?;
    let edit_menu = SubmenuBuilder::with_id(app, "edit", "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .separator()
        .item(&find)
        .item(&find_next)
        .item(&find_previous)
        .item(&replace)
        .build()?;

    let command_palette = command(
        app,
        "view.commandPalette",
        "Command Palette…",
        Some("CmdOrCtrl+Shift+P"),
    )?;
    let quick_open = command(app, "view.quickOpen", "Quick Open…", Some("CmdOrCtrl+P"))?;
    let toggle_sidebar = check_command(
        app,
        "view.toggleSidebar",
        "Toggle Sidebar",
        Some("CmdOrCtrl+B"),
    )?;
    let toggle_results = command(app, "view.toggleResults", "Toggle Results", None)?;
    let toggle_bottom_panel = command(
        app,
        "view.toggleBottomPanel",
        "Toggle Bottom Panel",
        Some("CmdOrCtrl+J"),
    )?;
    let open_settings_view = command(
        app,
        "view.openSettings",
        "Application Settings",
        Some("CmdOrCtrl+,"),
    )?;
    let appearance_light = check_command(app, "view.appearance.light", "Light", None)?;
    let appearance_dark = check_command(app, "view.appearance.dark", "Dark", None)?;
    let appearance_system = check_command(app, "view.appearance.system", "System", None)?;
    let appearance = SubmenuBuilder::with_id(app, "view-appearance", "Appearance")
        .item(&appearance_light)
        .item(&appearance_dark)
        .item(&appearance_system)
        .build()?;
    let zoom_in = command(app, "view.zoomIn", "Zoom In", Some("CmdOrCtrl+="))?;
    let zoom_out = command(app, "view.zoomOut", "Zoom Out", Some("CmdOrCtrl+-"))?;
    let reset_zoom = command(app, "view.resetZoom", "Reset Zoom", Some("CmdOrCtrl+0"))?;
    let view_menu = SubmenuBuilder::with_id(app, "view", "View")
        .item(&command_palette)
        .item(&quick_open)
        .separator()
        .item(&toggle_sidebar)
        .item(&toggle_results)
        .item(&toggle_bottom_panel)
        .item(&open_settings_view)
        .separator()
        .item(&appearance)
        .separator()
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&reset_zoom)
        .item(&PredefinedMenuItem::fullscreen(app, None)?)
        .build()?;

    let run_query = command(app, "query.run", "Run Query", Some("CmdOrCtrl+Enter"))?;
    let run_selection = command(app, "query.runSelection", "Run Selected Query", None)?;
    let stop_query = command(app, "query.stop", "Stop Query", None)?;
    let explain = command(app, "query.explain", "Explain", None)?;
    let explain_analyze = command(app, "query.explainAnalyze", "Explain Analyze", None)?;
    let format = command(app, "query.format", "Format SQL", None)?;
    let history = command(app, "query.history", "Query History…", None)?;
    let query_menu = SubmenuBuilder::with_id(app, "query", "Query")
        .item(&run_query)
        .item(&run_selection)
        .item(&stop_query)
        .separator()
        .item(&explain)
        .item(&explain_analyze)
        .separator()
        .item(&format)
        .item(&history)
        .build()?;

    let new_database_connection = command(
        app,
        "database.newConnection",
        "New Connection…",
        Some("CmdOrCtrl+Shift+N"),
    )?;
    let connect = command(app, "database.connect", "Connect", None)?;
    let disconnect = command(app, "database.disconnect", "Disconnect", None)?;
    let reconnect = command(app, "database.reconnect", "Reconnect", None)?;
    let active_connection = command(app, "database.activeConnection", "Active Connection", None)?;
    let refresh = command(app, "database.refresh", "Refresh", None)?;
    let refresh_schema = command(app, "database.refreshSchema", "Refresh Schema", None)?;
    let auto_commit = check_command(app, "database.autoCommit", "Auto-Commit", None)?;
    let commit = command(app, "database.commit", "Commit", None)?;
    let rollback = command(app, "database.rollback", "Rollback", None)?;
    let transaction = SubmenuBuilder::with_id(app, "database-transaction", "Transaction")
        .item(&auto_commit)
        .item(&commit)
        .item(&rollback)
        .build()?;
    let database_menu = SubmenuBuilder::with_id(app, "database", "Database")
        .item(&new_database_connection)
        .separator()
        .item(&connect)
        .item(&disconnect)
        .item(&reconnect)
        .separator()
        .item(&active_connection)
        .separator()
        .item(&refresh)
        .item(&refresh_schema)
        .separator()
        .item(&transaction)
        .build()?;

    let bring_all_to_front = PredefinedMenuItem::bring_all_to_front(app, None)?;
    let window_menu = SubmenuBuilder::with_id(app, "window", "Window")
        .item(&PredefinedMenuItem::minimize(app, None)?)
        .item(&PredefinedMenuItem::maximize(app, None)?)
        .item(&PredefinedMenuItem::fullscreen(app, None)?)
        .separator()
        .item(&bring_all_to_front)
        .build()?;

    let search = command(app, "help.search", "Search", None)?;
    let documentation = command(app, "help.documentation", "Orbit SQL Documentation", None)?;
    let keyboard_shortcuts = command(app, "help.keyboardShortcuts", "Keyboard Shortcuts", None)?;
    let report_issue = command(app, "help.reportIssue", "Report an Issue…", None)?;
    let check_updates = command(app, "help.checkForUpdates", "Check for Updates…", None)?;
    let about = command(app, "help.about", "About Orbit SQL", None)?;
    let help_menu = SubmenuBuilder::with_id(app, "help", "Help")
        .item(&search)
        .separator()
        .item(&documentation)
        .item(&keyboard_shortcuts)
        .separator()
        .item(&report_issue)
        .item(&check_updates)
        .separator()
        .item(&about)
        .build()?;

    MenuBuilder::new(app)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&query_menu)
        .item(&database_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()
}
