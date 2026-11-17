use orbitsql_ui_lib::backend::RealConnectionDriverFactory;
use orbitsql_ui_lib::backend::domain::catalog::DialectId;
use orbitsql_ui_lib::backend::domain::connection::{ConnectionAuthMethod, ConnectionProfileRecord};
use orbitsql_ui_lib::backend::ports::{ConnectionDriverFactory, ResolvedConnection};
use std::collections::BTreeMap;

#[derive(Clone)]
struct Profile {
    host: String,
    port: u16,
    user_name: String,
    password: String,
    database: String,
    auth_method: ConnectionAuthMethod,
}

fn profile(prefix: &str, dialect: DialectId) -> Option<Profile> {
    let value = |suffix: &str| std::env::var(format!("{prefix}_{suffix}")).ok();
    let auth_method = match (dialect, value("AUTH").as_deref()) {
        (DialectId::Sqlserver, Some("ntlm" | "NTLM" | "windows" | "WINDOWS")) => {
            ConnectionAuthMethod::WindowsNtlm
        }
        _ => ConnectionAuthMethod::Password,
    };
    Some(Profile {
        host: value("HOST")?,
        port: value("PORT")?.parse().ok()?,
        user_name: value("USER")?,
        password: value("PASSWORD")?,
        database: value("DATABASE")?,
        auth_method,
    })
}

fn runtime_profile(profile: &Profile, dialect: DialectId) -> ConnectionProfileRecord {
    let mut driver_properties = BTreeMap::new();
    if matches!(dialect, DialectId::Postgresql) {
        driver_properties.insert("sslMode".to_owned(), "disable".to_owned());
    }
    ConnectionProfileRecord {
        id: format!("result-session:{dialect:?}"),
        name: "result-session-integration".to_owned(),
        dialect: dialect.clone(),
        server: profile.host.clone(),
        port: profile.port,
        user_name: profile.user_name.clone(),
        database: profile.database.clone(),
        trust_server_certificate: matches!(dialect, DialectId::Sqlserver),
        auth_method: profile.auth_method.clone(),
        profile_id: "integration".to_owned(),
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        updated_at: "2026-01-01T00:00:00Z".to_owned(),
        show_all_catalog_items: true,
        transport: Default::default(),
        driver_properties,
        keep_alive_enabled: false,
        keep_alive_interval_seconds: 15,
        idle_timeout_seconds: None,
    }
}

async fn assert_batches(profile: Profile, dialect: DialectId, sql: &str) {
    let factory = RealConnectionDriverFactory;
    let driver = factory.driver(dialect.clone()).expect("driver must exist");
    let connection = driver
        .connect(ResolvedConnection {
            profile: runtime_profile(&profile, dialect),
            password: profile.password,
        })
        .await
        .expect("connection must be established");
    let result = connection
        .start_result_session(sql.to_owned(), 200)
        .await
        .expect("result session must start");
    assert_eq!(result.result.first_batch.row_count, 200);
    assert!(result.result.first_batch.has_more);
    assert_eq!(result.result.first_batch.total_fetched, 200);
    assert_row_range(&result.result.first_batch.rows, 1, 200);

    let second = result
        .session
        .fetch_next(200)
        .await
        .expect("second batch must use the same execution");
    assert_eq!(second.row_count, 200);
    assert!(second.has_more);
    assert_eq!(second.total_fetched, 400);
    assert_row_range(&second.rows, 201, 400);

    let third = result
        .session
        .fetch_next(200)
        .await
        .expect("third batch must use the same execution");
    assert_eq!(third.row_count, 200);
    assert!(!third.has_more);
    assert_eq!(third.total_fetched, 600);
    assert_row_range(&third.rows, 401, 600);

    result.session.close().await.expect("session must close");
    let small_batch = connection
        .start_result_session(sql.to_owned(), 6)
        .await
        .expect("a small result session must start");
    for _ in 0..12 {
        let batch = small_batch
            .session
            .fetch_next(6)
            .await
            .expect("small result session must fetch");
        if !batch.has_more {
            break;
        }
    }
    small_batch
        .session
        .close()
        .await
        .expect("small result session must close");
    let concurrent = connection
        .start_result_session(sql.to_owned(), 6)
        .await
        .expect("a concurrent result session must start");
    let fetch_session = concurrent.session.clone();
    let fetch_task = tokio::spawn(async move { fetch_session.fetch_next(6).await });
    concurrent
        .session
        .close()
        .await
        .expect("concurrent result session must close");
    let _ = fetch_task.await.expect("fetch task must finish");
    let cancelled = connection
        .start_result_session(sql.to_owned(), 200)
        .await
        .expect("a second result session must start");
    cancelled
        .session
        .fetch_next(200)
        .await
        .expect("the cancellable session must fetch");
    cancelled
        .session
        .cancel()
        .await
        .expect("the result source must cancel");
    let reopened = connection
        .start_result_session("SELECT 1 AS id".to_owned(), 1)
        .await
        .expect("the connection must be reusable after cancellation");
    assert_eq!(reopened.result.first_batch.row_count, 1);
    reopened.session.close().await.expect("session must close");
    connection.close().await.expect("connection must close");
}

async fn assert_sqlserver_error_releases_connection(profile: Profile) {
    let factory = RealConnectionDriverFactory;
    let driver = factory
        .driver(DialectId::Sqlserver)
        .expect("driver must exist");
    let connection = driver
        .connect(ResolvedConnection {
            profile: runtime_profile(&profile, DialectId::Sqlserver),
            password: profile.password,
        })
        .await
        .expect("connection must be established");
    let failed = connection
        .start_result_session("SELECT * FROM [[trinus-uau].customer".to_owned(), 200)
        .await;
    assert!(failed.is_err());
    let recovered = connection
        .start_result_session("SELECT 1 AS id".to_owned(), 1)
        .await
        .expect("the connection must be reusable after a SQL Server error");
    assert_eq!(recovered.result.first_batch.row_count, 1);
    recovered
        .session
        .close()
        .await
        .expect("recovered session must close");
    connection.close().await.expect("connection must close");
}

fn assert_row_range(rows: &[Vec<serde_json::Value>], first: i64, last: i64) {
    assert_eq!(
        rows.first()
            .and_then(|row| row.first())
            .and_then(serde_json::Value::as_i64),
        Some(first)
    );
    assert_eq!(
        rows.last()
            .and_then(|row| row.first())
            .and_then(serde_json::Value::as_i64),
        Some(last)
    );
}

#[tokio::test]
#[ignore = "requires ORBITSQL_TEST_POSTGRES_* integration profile"]
async fn postgres_result_session_reads_three_batches_without_rerunning_sql() {
    let profile = profile("ORBITSQL_TEST_POSTGRES", DialectId::Postgresql)
        .expect("ORBITSQL_TEST_POSTGRES_* must be configured for this test");
    assert_batches(
        profile,
        DialectId::Postgresql,
        "SELECT generate_series(1, 600) AS id",
    )
    .await;
}

#[tokio::test]
#[ignore = "requires ORBITSQL_TEST_SQLSERVER_* integration profile"]
async fn sqlserver_result_session_reads_three_batches_without_rerunning_sql() {
    let profile = profile("ORBITSQL_TEST_SQLSERVER", DialectId::Sqlserver)
        .expect("ORBITSQL_TEST_SQLSERVER_* must be configured for this test");
    assert_sqlserver_error_releases_connection(profile.clone()).await;
    assert_batches(
        profile,
        DialectId::Sqlserver,
        "WITH numbers AS (SELECT CAST(1 AS INT) AS id UNION ALL SELECT id + 1 FROM numbers WHERE id < 600) SELECT id FROM numbers OPTION (MAXRECURSION 0)",
    )
    .await;
}
