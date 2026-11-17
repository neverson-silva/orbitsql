use orbitsql_ui_lib::backend::catalog::DialectId;
use orbitsql_ui_lib::backend::domain::connection::{ConnectionAuthMethod, ConnectionProfileRecord};

#[test]
fn persisted_connection_profile_contains_no_secret_field() {
    let profile = ConnectionProfileRecord {
        id: "7f5e9a10-87e5-45e4-b6c8-5b6bb8a8ad30".to_owned(),
        name: "Local SQL Server".to_owned(),
        dialect: DialectId::Sqlserver,
        server: "localhost".to_owned(),
        port: 1433,
        user_name: "sa".to_owned(),
        database: "master".to_owned(),
        trust_server_certificate: true,
        auth_method: ConnectionAuthMethod::Password,
        profile_id: "Development".to_owned(),
        created_at: "1".to_owned(),
        updated_at: "1".to_owned(),
        show_all_catalog_items: false,
        transport: Default::default(),
        driver_properties: Default::default(),
        keep_alive_enabled: true,
        keep_alive_interval_seconds: 15,
        idle_timeout_seconds: None,
    };
    let value = serde_json::to_value(profile).expect("profile must serialize");
    assert!(value.get("password").is_none());
    assert_eq!(value["dialect"], "sqlserver");
    assert_eq!(value["authMethod"], "password");
}
