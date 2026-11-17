use orbitsql_ui_lib::backend::domain::connection::{ConnectionAuthMethod, ConnectionProfileRecord};
use orbitsql_ui_lib::backend::ports::{ConnectionDriverFactory, ResolvedConnection};
use orbitsql_ui_lib::backend::{
    CatalogRefreshRequest, DatabaseDriverFactory, RealConnectionDriverFactory,
    RefreshCatalogUseCase,
};
use orbitsql_ui_lib::catalog::{CatalogSnapshot, DialectId};
use std::collections::BTreeMap;

#[derive(Clone)]
struct Profile {
    host: String,
    port: u16,
    user_name: String,
    password: String,
    database: String,
}

fn profile(prefix: &str) -> Option<Profile> {
    let value = |suffix: &str| std::env::var(format!("{prefix}_{suffix}")).ok();
    Some(Profile {
        host: value("HOST")?,
        port: value("PORT")?.parse().ok()?,
        user_name: value("USER")?,
        password: value("PASSWORD")?,
        database: value("DATABASE")?,
    })
}

fn request(profile: &Profile, dialect: DialectId, name: &str) -> CatalogRefreshRequest {
    CatalogRefreshRequest {
        dialect,
        connection_id: format!("integration:{name}"),
        version: "integration-v1".to_owned(),
        generated_at: "2026-01-01T00:00:00Z".to_owned(),
        server: profile.host.clone(),
        port: profile.port,
        user_name: profile.user_name.clone(),
        password: profile.password.clone(),
        database: profile.database.clone(),
        trust_server_certificate: true,
        auth_method: None,
        show_all_catalog_items: true,
        transport: Default::default(),
        driver_properties: Default::default(),
    }
}

async fn refresh(
    profile: &Profile,
    dialect: DialectId,
    name: &str,
) -> Result<CatalogSnapshot, String> {
    refresh_request(request(profile, dialect, name)).await
}

async fn refresh_request(request: CatalogRefreshRequest) -> Result<CatalogSnapshot, String> {
    RefreshCatalogUseCase::new(DatabaseDriverFactory::default())
        .execute(request)
        .await
        .map_err(|error| error.to_string())
}

fn assert_fixture(snapshot: CatalogSnapshot, dialect: DialectId) {
    assert_eq!(snapshot.dialect, dialect);
    assert_eq!(snapshot.schema_version, 1);
    let users = snapshot
        .tables
        .iter()
        .find(|table| table.name.normalized == "completion_users")
        .expect("provider must return the fixture table");
    assert_eq!(
        users.catalog_id.as_deref(),
        Some(snapshot.database_id.as_str())
    );
    assert!(
        users
            .columns
            .iter()
            .any(|column| column.name.normalized == "id")
    );
    assert!(users.columns.iter().any(|column| {
        column.name.normalized == "email" && !column.unique_memberships.is_empty()
    }));
    assert!(
        snapshot
            .indexes
            .iter()
            .any(|index| index.table_id == users.id && index.unique)
    );
    let orders = snapshot
        .tables
        .iter()
        .find(|table| table.name.normalized == "completion_orders")
        .expect("provider must return the relationship table");
    assert!(snapshot.foreign_keys.iter().any(|foreign_key| {
        foreign_key.source_table_id == orders.id && foreign_key.target_table_id == users.id
    }));
    assert!(orders.check_constraints.iter().any(|constraint| {
        constraint
            .expression
            .as_deref()
            .is_some_and(|expression| expression.to_ascii_lowercase().contains("total"))
    }));
    assert!(orders.columns.iter().any(|column| {
        column.name.normalized == "user_id" && !column.foreign_key_memberships.is_empty()
    }));
    assert!(
        snapshot
            .functions
            .iter()
            .any(|function| function.name.normalized == "completion_identity")
    );
    assert!(
        snapshot
            .procedures
            .iter()
            .any(|procedure| procedure.name.normalized == "completion_noop")
    );
}

#[tokio::test]
#[ignore = "requires ORBITSQL_TEST_POSTGRES_* integration profile"]
async fn postgres_provider_normalizes_catalog_metadata() {
    let profile = profile("ORBITSQL_TEST_POSTGRES")
        .expect("ORBITSQL_TEST_POSTGRES_* must be configured for this test");
    let snapshot = refresh(&profile, DialectId::Postgresql, "postgres")
        .await
        .expect("PostgreSQL provider must introspect the configured fixture");
    assert_fixture(snapshot, DialectId::Postgresql);
}

#[tokio::test]
#[ignore = "requires ORBITSQL_TEST_SQLSERVER_* integration profile"]
async fn sqlserver_provider_normalizes_catalog_metadata_with_tedious_rs() {
    let profile = profile("ORBITSQL_TEST_SQLSERVER")
        .expect("ORBITSQL_TEST_SQLSERVER_* must be configured for this test");
    let snapshot = refresh(&profile, DialectId::Sqlserver, "sqlserver")
        .await
        .expect("SQL Server provider must introspect the configured database");
    assert_eq!(snapshot.dialect, DialectId::Sqlserver);
    assert_eq!(snapshot.schema_version, 1);
    assert!(
        snapshot
            .schemas
            .iter()
            .any(|schema| schema.name.normalized == "dbo")
    );
}

#[tokio::test]
#[ignore = "requires ORBITSQL_TEST_SQLSERVER_* integration profile"]
async fn sqlserver_session_telemetry_reads_live_dmvs_through_tedious_rs() {
    let profile = profile("ORBITSQL_TEST_SQLSERVER")
        .expect("ORBITSQL_TEST_SQLSERVER_* must be configured for this test");
    let factory = RealConnectionDriverFactory;
    let driver = factory
        .driver(DialectId::Sqlserver)
        .expect("SQL Server driver must be registered");
    let connection = driver
        .connect(ResolvedConnection {
            profile: ConnectionProfileRecord {
                id: "session-telemetry-integration".to_owned(),
                name: "session-telemetry-integration".to_owned(),
                dialect: DialectId::Sqlserver,
                server: profile.host,
                port: profile.port,
                user_name: profile.user_name,
                database: profile.database,
                trust_server_certificate: true,
                auth_method: ConnectionAuthMethod::Password,
                profile_id: "integration".to_owned(),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                show_all_catalog_items: true,
                transport: Default::default(),
                driver_properties: BTreeMap::new(),
                keep_alive_enabled: false,
                keep_alive_interval_seconds: 15,
                idle_timeout_seconds: None,
            },
            password: profile.password,
        })
        .await
        .expect("SQL Server connection must be established with tedious-rs");
    let telemetry = connection
        .session_telemetry()
        .await
        .expect("SQL Server DMVs must return live session telemetry");
    assert!(telemetry.monitor_session_id > 0);
    assert!(telemetry.sessions.iter().any(|session| {
        session.session_id == telemetry.monitor_session_id
            && session
                .batch_text
                .as_deref()
                .is_some_and(|batch| batch.contains("sys.dm_exec_sessions"))
    }));
    assert!(!telemetry.captured_at.is_empty());
    connection
        .close()
        .await
        .expect("SQL Server telemetry connection must close");
}

#[tokio::test]
#[ignore = "requires ORBITSQL_TEST_POSTGRES_* integration profile"]
async fn postgres_provider_rejects_untrusted_fixture_without_tls() {
    let profile = profile("ORBITSQL_TEST_POSTGRES")
        .expect("ORBITSQL_TEST_POSTGRES_* must be configured for this test");
    let mut request = request(&profile, DialectId::Postgresql, "postgres-untrusted");
    request.trust_server_certificate = false;
    assert!(refresh_request(request).await.is_err());
}
