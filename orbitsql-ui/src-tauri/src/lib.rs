pub mod backend;

#[cfg(target_os = "macos")]
mod native_menu;

use std::time::Duration;
use tauri::{Emitter, Manager};

pub use backend::catalog;
pub use backend::ports::{
    CatalogProvider, CatalogProviderError, CatalogProviderFactory, CatalogRefreshRequest,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(backend::DatabaseDriverFactory::default())
        .manage(backend::RealConnectionDriverFactory)
        .manage(backend::ConnectionRuntimeRegistry::default())
        .manage(backend::QueryCancellationRegistry::default())
        .manage(backend::ResultSessionRegistry::default())
        .manage(backend::CatalogSnapshotCache::default())
        .invoke_handler(tauri::generate_handler![
            backend::commands::test_connection,
            backend::commands::save_connection,
            backend::commands::save_connection_profile,
            backend::commands::list_connection_profiles,
            backend::commands::list_connection_profile_definitions,
            backend::commands::upsert_connection_profile_definition,
            backend::commands::delete_connection_profile_definition,
            backend::commands::list_connection_statuses,
            backend::commands::ping_connection,
            backend::commands::connect_connection,
            backend::commands::connect_connection_only,
            backend::commands::disconnect_connection,
            backend::commands::delete_connection,
            backend::commands::update_connection,
            backend::commands::refresh_connection_catalog,
            backend::commands::expand_connection_node,
            backend::commands::rename_connection,
            backend::commands::execute_query,
            backend::commands::execute_query_in_context,
            backend::commands::start_query_session,
            backend::commands::start_query_session_in_context,
            backend::commands::fetch_query_session,
            backend::commands::cancel_query_session,
            backend::commands::close_query_session,
            backend::commands::cancel_query,
            backend::commands::get_database_sessions,
            backend::commands::kill_database_session,
            backend::commands::secret_store_status,
            backend::commands::create_secret_vault,
            backend::commands::unlock_secret_vault,
            backend::commands::lock_secret_vault,
            backend::commands::sync_menu_state
        ])
        .setup(|app| {
            app.manage(backend::adapters::resolve_secret_store(app.handle())?);
            let monitor_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let registry = monitor_app.state::<backend::ConnectionRuntimeRegistry>();
                    for (connection_id, error) in registry.check_health().await {
                        let payload = serde_json::json!({
                            "connectionId": connection_id,
                            "status": "disconnected",
                            "errorCode": error.code,
                        });
                        let _ = monitor_app.emit("connection-health", payload);
                    }
                }
            });
            #[cfg(target_os = "macos")]
            {
                let menu = native_menu::build(app.handle())?;
                app.set_menu(menu)?;
            }

            #[cfg(all(desktop, not(target_os = "macos")))]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_decorations(false)?;
                }
            }

            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = app.emit("application-command", event.id().as_ref().to_string());
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");
    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            let sessions = app_handle.state::<backend::ResultSessionRegistry>();
            tauri::async_runtime::block_on(sessions.close_all());
            let registry = app_handle.state::<backend::ConnectionRuntimeRegistry>();
            let _ = tauri::async_runtime::block_on(registry.close_all());
        }
    });
}
