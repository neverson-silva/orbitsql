use std::sync::{Arc, Mutex};
use std::time::Duration;
use tedious_rs::connection::{Connection, ConnectionEvent};
use tedious_rs::request::Request;
use tedious_rs::token::stream::StreamToken;
use tedious_rs::value_codec::Value;

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_rejects_invalid_credentials_without_leaking_connection_state()
-> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::common::sql_config()?;
    config.authentication = tedious_rs::Authentication::Sql {
        user_name: "sa".to_owned(),
        password: "invalid-password".to_owned(),
    };
    let mut connection = Connection::new(config);
    let error = connection.connect().await.unwrap_err();
    assert!(error.code.is_some());
    assert_eq!(connection.state, tedious_rs::ConnectionState::Final);
    Ok(())
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_rejects_missing_database_with_terminal_connection_state()
-> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::common::sql_config()?;
    config.database = Some("tedious_rs_database_that_does_not_exist".to_owned());
    let mut connection = Connection::new(config);
    let error = connection.connect().await.unwrap_err();
    assert!(error.code.is_some());
    assert_eq!(connection.state, tedious_rs::ConnectionState::Final);
    Ok(())
}

#[tokio::test]
#[ignore = "requires the configured local SQL Server Authentication endpoint"]
async fn sql_auth_connection_executes_metadata_and_crud() -> Result<(), Box<dyn std::error::Error>>
{
    let table = crate::common::sql_auth_object_name("port_test");
    let qualified_table = format!("dbo.{table}");
    let cleanup = format!(
        "IF OBJECT_ID(N'{qualified_table}', N'U') IS NOT NULL DROP TABLE {qualified_table};"
    );
    crate::common::with_sql_connection_cleanup(cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!(
                    "CREATE TABLE {qualified_table} (id int NOT NULL, name nvarchar(100) NULL); INSERT INTO {qualified_table} (id, name) VALUES (1, N'linha');"
                ))
                .await?;
            let selected = connection
                .exec_sql_batch(format!("SELECT id, name FROM {qualified_table};"))
                .await?;
            assert!(selected.iter().any(|token| matches!(
                token,
                StreamToken::ColMetadata(columns) if columns.len() == 2
            )));
            assert!(selected.iter().any(|token| matches!(
                token,
                StreamToken::Row(row) if row.columns.len() == 2
            )));
            connection
                .exec_sql_batch(format!(
                    "UPDATE {qualified_table} SET name = N'atualizada' WHERE id = 1; DELETE FROM {qualified_table} WHERE id = 1;"
                ))
                .await?;
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires the configured local SQL Authentication endpoint"]
async fn sql_auth_connection_emits_the_complete_connect_state_sequence()
-> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::common::sql_config()?;
    config.debug.payload = true;
    let mut connection = Connection::new(config);
    let states = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&states);
    connection.on(move |event| {
        if let ConnectionEvent::Debug(message) = event
            && message.starts_with("State change:")
        {
            observed.lock().unwrap().push(message.clone());
        }
    });
    connection.connect().await?;
    connection.close().await;
    let states = states.lock().unwrap().join("\n");
    for state in [
        "Initialized -> Connecting",
        "Connecting -> SentPrelogin",
        "SentPrelogin -> SentTLSSSLNegotiation",
        "SentTLSSSLNegotiation -> SentLogin7WithStandardLogin",
        "SentLogin7WithStandardLogin -> LoggedInSendingInitialSql",
        "LoggedInSendingInitialSql -> LoggedIn",
    ] {
        assert!(
            states.contains(state),
            "missing state transition: {state}; observed: {states}"
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_cancellation_of_active_request_preserves_authenticated_connection()
-> Result<(), Box<dyn std::error::Error>> {
    crate::common::with_sql_connection(|connection| {
        Box::pin(async move {
            let mut request = Request::new("WAITFOR DELAY '00:00:05'; SELECT 1;", |_| {});
            let cancellation = request.cancellation_handle();
            let result = {
                let execution = connection.exec_sql_batch_request(&mut request);
                tokio::pin!(execution);
                tokio::select! {
                    result = &mut execution => result,
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        cancellation.cancel();
                        execution.await
                    }
                }
            };
            let error = result.unwrap_err();
            assert_eq!(error.code.as_deref(), Some("ECANCEL"));
            assert_eq!(
                connection.state,
                tedious_rs::connection::ConnectionState::LoggedIn
            );
            connection.exec_sql_batch("SELECT 1").await?;
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a running SQL Server with NTLM and TEST_DB_HOST, VPN_USER, VPN_PWD, TEST_DB_NAME"]
async fn read_only_connection_executes_select_and_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let config = crate::common::ntlm_config(None)?;
    let mut connection = Connection::new(config);
    connection.connect().await?;
    let selected = connection
        .exec_sql_batch("SELECT TABLE_SCHEMA, TABLE_NAME FROM INFORMATION_SCHEMA.TABLES;")
        .await?;
    assert!(
        selected.iter().any(|token| {
            matches!(token, StreamToken::ColMetadata(columns) if columns.len() == 2)
        })
    );
    assert!(
        selected
            .iter()
            .any(|token| { matches!(token, StreamToken::Row(row) if row.columns.len() == 2) })
    );
    connection.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires a running SQL Server with NTLM and TEST_DB_HOST, VPN_USER, VPN_PWD, TEST_DB_NAME"]
async fn ntlm_connection_without_domain_executes_select_only()
-> Result<(), Box<dyn std::error::Error>> {
    let config = crate::common::ntlm_config(None)?;
    let mut connection = Connection::new(config);
    connection.connect().await?;
    let selected = connection.exec_sql_batch("SELECT 1 AS value;").await?;
    assert!(
        selected
            .iter()
            .any(|token| matches!(token, StreamToken::Row(row) if row.columns.len() == 1))
    );
    connection.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires a running SQL Server with NTLM and TEST_DB_HOST, VPN_USER, VPN_PWD, TEST_DB_NAME; VPN_DOMAIN is optional"]
async fn ntlm_connection_accepts_optional_domain_from_environment()
-> Result<(), Box<dyn std::error::Error>> {
    let config = crate::common::ntlm_config_from_environment()?;
    let mut connection = Connection::new(config);
    connection.connect().await?;
    let selected = connection.exec_sql_batch("SELECT 1 AS value;").await?;
    assert!(
        selected
            .iter()
            .any(|token| matches!(token, StreamToken::Row(row) if row.columns.len() == 1))
    );
    connection.close().await;
    Ok(())
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_reads_object_metadata_and_executes_parameterized_crud()
-> Result<(), Box<dyn std::error::Error>> {
    let table = crate::common::sql_auth_object_name("metadata_test");
    let qualified_table = format!("dbo.{table}");
    let cleanup = format!(
        "IF OBJECT_ID(N'{qualified_table}', N'U') IS NOT NULL DROP TABLE {qualified_table};"
    );
    crate::common::with_sql_connection_cleanup(cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!(
                    "CREATE TABLE {qualified_table} (id int NOT NULL PRIMARY KEY, name nvarchar(100) NULL);"
                ))
                .await?;
            let metadata = connection
                .exec_sql_batch(format!(
                    "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_NAME = '{table}' ORDER BY ORDINAL_POSITION;"
                ))
                .await?;
            assert!(metadata.iter().any(|token| {
                matches!(token, StreamToken::ColMetadata(columns) if columns.len() == 3)
            }));
            let mut insert = Request::new(
                format!(
                    "INSERT INTO {qualified_table} (id, name) VALUES (@id, @name);"
                ),
                |_| {},
            );
            insert.add_typed_parameter(
                "id",
                tedious_rs::parameter::SqlParameterType::Int,
                Value::Integer(1),
                tedious_rs::parameter::ParameterOptions::default(),
            )?;
            insert.add_typed_parameter(
                "name",
                tedious_rs::parameter::SqlParameterType::NVarChar,
                Value::Text("um".to_owned()),
                tedious_rs::parameter::ParameterOptions {
                    length: Some(100),
                    ..Default::default()
                },
            )?;
            connection.exec_sql(&mut insert).await?;
            let mut update = Request::new(
                format!("UPDATE {qualified_table} SET name = @name WHERE id = @id;"),
                |_| {},
            );
            update.add_typed_parameter(
                "id",
                tedious_rs::parameter::SqlParameterType::Int,
                Value::Integer(1),
                tedious_rs::parameter::ParameterOptions::default(),
            )?;
            update.add_typed_parameter(
                "name",
                tedious_rs::parameter::SqlParameterType::NVarChar,
                Value::Text("dois".to_owned()),
                tedious_rs::parameter::ParameterOptions {
                    length: Some(100),
                    ..Default::default()
                },
            )?;
            connection.exec_sql(&mut update).await?;
            let selected = connection
                .exec_sql_batch(format!(
                    "SELECT id, name FROM {qualified_table} WHERE id = 1;"
                ))
                .await?;
            assert!(selected
                .iter()
                .any(|token| matches!(token, StreamToken::Row(row) if row.columns.len() == 2)));
            connection
                .exec_sql_batch(format!("DELETE FROM {qualified_table};"))
                .await?;
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_preserves_multiple_result_sets_and_transaction_rollback()
-> Result<(), Box<dyn std::error::Error>> {
    let table = crate::common::sql_auth_object_name("transaction_test");
    let qualified_table = format!("dbo.{table}");
    let cleanup = format!(
        "IF OBJECT_ID(N'{qualified_table}', N'U') IS NOT NULL DROP TABLE {qualified_table};"
    );
    crate::common::with_sql_connection_cleanup(cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!(
                    "CREATE TABLE {qualified_table} (id int NOT NULL);"
                ))
                .await?;
            connection
                .begin_transaction(
                    "tedious_rs_tx",
                    tedious_rs::transaction::IsolationLevel::ReadCommitted,
                )
                .await?;
            connection
                .exec_sql_batch(format!(
                    "INSERT INTO {qualified_table} (id) VALUES (1);"
                ))
                .await?;
            connection.rollback_transaction("tedious_rs_tx").await?;
            let result_sets = connection
                .exec_sql_batch("SELECT 1 AS first_set; SELECT 2 AS second_set;")
                .await?;
            assert_eq!(
                result_sets
                    .iter()
                    .filter(|token| matches!(token, StreamToken::ColMetadata(_)))
                    .count(),
                2
            );
            let persisted = connection
                .exec_sql_batch(format!(
                    "SELECT COUNT(*) AS total FROM {qualified_table};"
                ))
                .await?;
            assert!(persisted.iter().any(|token| {
                matches!(token, StreamToken::Row(row) | StreamToken::NbcRow(row) if row.columns.len() == 1)
            }));
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_transaction_commit_and_savepoint_preserve_descriptor_lifecycle()
-> Result<(), Box<dyn std::error::Error>> {
    let table = crate::common::sql_auth_object_name("transaction_commit");
    let qualified_table = format!("dbo.{table}");
    let cleanup = format!(
        "IF OBJECT_ID(N'{qualified_table}', N'U') IS NOT NULL DROP TABLE {qualified_table};"
    );
    crate::common::with_sql_connection_cleanup(cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!("CREATE TABLE {qualified_table} (id int NOT NULL);"))
                .await?;
            connection
                .begin_transaction(
                    "tedious_rs_commit",
                    tedious_rs::transaction::IsolationLevel::ReadCommitted,
                )
                .await?;
            connection
                .exec_sql_batch(format!(
                    "INSERT INTO {qualified_table} (id) VALUES (1);"
                ))
                .await?;
            connection.save_transaction("tedious_rs_savepoint").await?;
            connection
                .exec_sql_batch(format!(
                    "INSERT INTO {qualified_table} (id) VALUES (2);"
                ))
                .await?;
            connection.commit_transaction("tedious_rs_commit").await?;
            assert_eq!(connection.transaction_depth, 0);
            assert_eq!(connection.current_transaction_descriptor(), &[0; 8]);
            let committed = connection
                .exec_sql_batch(format!(
                    "SELECT COUNT(*) AS total FROM {qualified_table};"
                ))
                .await?;
            assert!(committed.iter().any(|token| {
                matches!(token, StreamToken::Row(row) | StreamToken::NbcRow(row) if row.columns.len() == 1)
            }));
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_reads_udt_values_and_nulls_as_binary_plp()
-> Result<(), Box<dyn std::error::Error>> {
    crate::common::with_sql_connection(|connection| {
        Box::pin(async move {
            let tokens = connection
                .exec_sql_batch(
                    "SELECT geography::STGeomFromText('LINESTRING(-122.360 47.656, -122.343 47.656 )', 4326) AS geo UNION ALL SELECT CAST(NULL AS geography);",
                )
                .await?;
            let rows = tokens
                .iter()
                .filter_map(|token| match token {
                    StreamToken::Row(row) | StreamToken::NbcRow(row) => Some(row),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 2);
            assert!(matches!(rows[0].columns[0].value, Value::Binary(_)));
            assert_eq!(rows[1].columns[0].value, Value::Null);
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_executes_rpc_and_prepared_request_lifecycle()
-> Result<(), Box<dyn std::error::Error>> {
    let procedure = crate::common::sql_auth_object_name("echo");
    let qualified_procedure = format!("dbo.{procedure}");
    let cleanup = format!(
        "IF OBJECT_ID(N'{qualified_procedure}', N'P') IS NOT NULL DROP PROCEDURE {qualified_procedure};"
    );
    let mut config = crate::common::sql_config()?;
    config.row_collection_on_done = true;
    config.row_collection_on_request_completion = true;
    crate::common::with_sql_connection_cleanup_with_config(config, cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!(
                    "CREATE PROCEDURE {qualified_procedure} @value int AS SELECT @value AS value;"
                ))
                .await?;
            let mut request = Request::new(qualified_procedure.clone(), |_| {});
            request.add_typed_parameter(
                "value",
                tedious_rs::parameter::SqlParameterType::Int,
                Value::Integer(42),
                tedious_rs::parameter::ParameterOptions::default(),
            )?;
            connection.call_procedure(&mut request).await?;
            assert_eq!(request.state, tedious_rs::RequestState::Completed);
            assert!(request.rows.iter().any(|row| {
                matches!(
                    row.columns.first().map(|column| &column.value),
                    Some(Value::Integer(42))
                )
            }));
            let mut prepared = Request::new("SELECT @value AS value", |_| {});
            prepared.add_typed_parameter(
                "value",
                tedious_rs::parameter::SqlParameterType::Int,
                Value::Integer(1),
                tedious_rs::parameter::ParameterOptions::default(),
            )?;
            connection.prepare(&mut prepared).await?;
            assert!(prepared.handle.is_some());
            connection
                .execute_prepared_with_values(
                    &mut prepared,
                    &[("value".to_owned(), Value::Integer(7))],
                )
                .await?;
            assert!(prepared.rows.iter().any(|row| {
                matches!(
                    row.columns.first().map(|column| &column.value),
                    Some(Value::Integer(7))
                )
            }));
            connection.unprepare(&mut prepared).await?;
            assert_eq!(prepared.handle, Some(1));
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_prepare_propagates_server_error_to_result_and_request()
-> Result<(), Box<dyn std::error::Error>> {
    crate::common::with_sql_connection(|connection| {
        Box::pin(async move {
            let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let observed = std::sync::Arc::clone(&events);
            let mut request =
                Request::new("SELECT * FROM dbo.__tedious_rs_missing_object__", |_| {});
            request.on(move |event| observed.lock().unwrap().push(event.clone()));
            let error = connection.prepare(&mut request).await.unwrap_err();
            assert_eq!(error.code.as_deref(), Some("EREQUEST"));
            assert!(request.error.is_some());
            assert_eq!(request.state, tedious_rs::RequestState::Created);
            assert!(
                events
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|event| matches!(event, tedious_rs::RequestEvent::Error(_)))
            );
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_resets_the_authenticated_connection_and_emits_once()
-> Result<(), Box<dyn std::error::Error>> {
    crate::common::with_sql_connection(|connection| {
        Box::pin(async move {
            let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let observed = std::sync::Arc::clone(&events);
            connection.on(move |event| {
                if matches!(
                    event,
                    tedious_rs::connection::ConnectionEvent::ResetConnection
                ) {
                    observed.lock().unwrap().push(true);
                }
            });
            connection.reset().await?;
            let selected = connection
                .exec_sql_batch("SELECT @@TRANCOUNT AS transaction_depth;")
                .await?;
            assert!(selected.iter().any(|token| {
                matches!(token, StreamToken::Row(row) | StreamToken::NbcRow(row) if row.columns.len() == 1)
            }));
            assert_eq!(events.lock().unwrap().len(), 1);
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a running local SQL Server Authentication endpoint"]
async fn sql_auth_request_timeout_stops_after_first_response_packet()
-> Result<(), Box<dyn std::error::Error>> {
    let mut config = crate::common::sql_config()?;
    config.request_timeout = Duration::from_millis(250);
    crate::common::with_sql_connection_cleanup_with_config(config, String::new(), |connection| {
        Box::pin(async move {
            let tokens = connection
                .exec_sql_batch(
                    "DECLARE @started datetime2 = SYSDATETIME(); SELECT 1 AS first_value; WAITFOR DELAY '00:00:01'; SELECT 2 AS second_value, DATEDIFF(MILLISECOND, @started, SYSDATETIME()) AS waited_ms;",
                )
                .await?;
            let waited_ms = tokens.iter().find_map(|token| match token {
                StreamToken::Row(row) | StreamToken::NbcRow(row) if row.columns.len() == 2 => {
                    row.columns.get(1).and_then(|column| match column.value {
                        Value::Integer(value) => Some(value),
                        _ => None,
                    })
                }
                _ => None,
            });
            assert!(waited_ms.is_some_and(|value| value >= 900));
            let values = tokens
                .iter()
                .filter_map(|token| match token {
                    StreamToken::Row(row) | StreamToken::NbcRow(row) => {
                        row.columns.first().map(|column| column.value.clone())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(values, vec![Value::Integer(1), Value::Integer(2)]);
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires a local SQL Server endpoint with encrypted TDS negotiation"]
async fn sql_auth_encrypted_tds_executes_select() -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::common::sql_config()?;
    let mut connection = Connection::new(config);
    let secure = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = std::sync::Arc::clone(&secure);
    connection.once_named("secure", move |_| {
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    connection.connect().await?;
    let tokens = connection.exec_sql_batch("SELECT 1;").await?;
    assert!(tokens.iter().any(|token| matches!(
        token,
        StreamToken::Row(row) if row.columns.len() == 1
    )));
    assert_eq!(secure.load(std::sync::atomic::Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
#[ignore = "requires the configured NTLM endpoint supporting strict TLS and TDS 8.0"]
async fn ntlm_strict_tls_tds80_executes_read_only_select() -> Result<(), Box<dyn std::error::Error>>
{
    let mut config = crate::common::ntlm_config_from_environment()?;
    config.strict_tls = true;
    config.tds_version = "8_0".to_owned();
    config.read_only = true;
    let mut connection = Connection::new(config);
    match connection.connect().await {
        Ok(()) => {
            let tokens = connection.exec_sql_batch("SELECT 1;").await?;
            assert!(tokens.iter().any(|token| matches!(
                token,
                StreamToken::Row(row) if row.columns.len() == 1
            )));
        }
        Err(error) => assert_eq!(error.code.as_deref(), Some("ESOCKET")),
    }
    Ok(())
}
