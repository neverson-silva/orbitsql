use futures_util::stream;
use std::time::Duration;
use tedious_rs::bulk_load::BulkLoadOptions;
use tedious_rs::connection::Connection;
use tedious_rs::token::parser::Token;
use tedious_rs::token::stream::StreamToken;
use tedious_rs::value_codec::Value;

fn add_id_column(bulk: &mut tedious_rs::bulk_load::BulkLoad) -> Result<(), String> {
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").ok_or("missing Int type")?,
        Default::default(),
        false,
    )
}

fn add_name_column(bulk: &mut tedious_rs::bulk_load::BulkLoad) -> Result<(), String> {
    bulk.add_typed_column(
        "name",
        tedious_rs::data_type::type_by_name("NVarChar").ok_or("missing NVarChar type")?,
        tedious_rs::parameter::ParameterOptions {
            length: Some(100),
            ..Default::default()
        },
        true,
    )
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and the local SQL Authentication endpoint"]
async fn sql_auth_bulk_load_supports_object_rows_and_stream_rows()
-> Result<(), Box<dyn std::error::Error>> {
    if !crate::common::integration_enabled() {
        return Ok(());
    }
    let mut config = crate::common::sql_config()?;
    config.row_collection_on_done = true;
    let mut connection = Connection::new(config);
    connection
        .connect()
        .await
        .map_err(|error| format!("connect: {error}"))?;
    let object_table = crate::common::sql_auth_object_name("bulk_object");
    let stream_table = crate::common::sql_auth_object_name("bulk_stream");
    let setup = connection
        .exec_sql_batch(format!(
            "CREATE TABLE {object_table} (id int NOT NULL, label nvarchar(50) NULL); CREATE TABLE {stream_table} (id int NOT NULL, label nvarchar(50) NULL);"
        ))
        .await
        .map_err(|error| format!("setup: {error}"));
    let result = async {
        setup.map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
        let mut object_bulk = connection.new_bulk_load(
            &object_table,
            BulkLoadOptions {
                keep_nulls: true,
                ..Default::default()
            },
        )?;
        object_bulk.add_typed_column(
            "id",
            tedious_rs::data_type::type_by_name("Int").ok_or("missing Int type")?,
            Default::default(),
            false,
        )?;
        object_bulk.add_typed_column_with_object_name(
            "label",
            "name",
            tedious_rs::data_type::type_by_name("NVarChar").ok_or("missing NVarChar type")?,
            tedious_rs::parameter::ParameterOptions {
                length: Some(50),
                ..Default::default()
            },
            true,
        )?;
        let mut first = std::collections::BTreeMap::new();
        first.insert("id".to_owned(), Value::Integer(10));
        first.insert("name".to_owned(), Value::Text("objeto".to_owned()));
        let mut second = std::collections::BTreeMap::new();
        second.insert("id".to_owned(), Value::Integer(11));
        second.insert("name".to_owned(), Value::Null);
        let object_tokens = connection
            .exec_bulk_load_objects(&mut object_bulk, vec![first, second])
            .await
            .map_err(|error| format!("object bulk execution: {error}"))?;
        assert!(!object_tokens.is_empty(), "bulk response was empty");

        let mut stream_bulk = connection.new_bulk_load(&stream_table, Default::default())?;
        stream_bulk.add_typed_column(
            "id",
            tedious_rs::data_type::type_by_name("Int").ok_or("missing Int type")?,
            Default::default(),
            false,
        )?;
        stream_bulk.add_typed_column(
            "label",
            tedious_rs::data_type::type_by_name("NVarChar").ok_or("missing NVarChar type")?,
            tedious_rs::parameter::ParameterOptions {
                length: Some(50),
                ..Default::default()
            },
            true,
        )?;
        let stream_tokens = connection
            .exec_bulk_load_value_stream(
                &mut stream_bulk,
                stream::iter(vec![
                    vec![Value::Integer(20), Value::Text("stream".to_owned())],
                    vec![Value::Integer(21), Value::Null],
                ]),
            )
            .await
            .map_err(|error| format!("stream bulk execution: {error}"))?;
        assert!(!stream_tokens.is_empty(), "stream response was empty");

        for (table, expected) in [(&object_table, 2_i64), (&stream_table, 2_i64)] {
            let tokens = connection
                .exec_sql_batch(format!("SELECT COUNT(*) AS total FROM {table};"))
                .await
                .map_err(|error| format!("count {table}: {error}"))?;
            assert!(tokens.iter().any(|token| {
                matches!(token, StreamToken::Row(row) | StreamToken::NbcRow(row) if row.columns.first().is_some_and(|column| column.value == Value::Integer(expected)))
            }));
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    let cleanup = connection
        .exec_sql_batch(format!(
            "IF OBJECT_ID(N'{object_table}', N'U') IS NOT NULL DROP TABLE {object_table}; IF OBJECT_ID(N'{stream_table}', N'U') IS NOT NULL DROP TABLE {stream_table};"
        ))
        .await
        .map_err(|error| format!("cleanup: {error}"));
    connection.close().await;
    match (result.err(), cleanup.err()) {
        (Some(error), _) => Err(error),
        (None, Some(error)) => Err(error.into()),
        (None, None) => Ok(()),
    }
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and the local SQL Authentication endpoint"]
async fn sql_auth_bulk_load_inserts_rows_and_preserves_nulls()
-> Result<(), Box<dyn std::error::Error>> {
    if !crate::common::integration_enabled() {
        return Ok(());
    }
    let mut config = crate::common::sql_config()?;
    config.row_collection_on_done = true;
    let mut connection = Connection::new(config);
    if let Err(error) = connection.connect().await {
        return Err(format!("connect: {error}").into());
    }
    let table = crate::common::sql_auth_object_name("bulk");
    let create = format!(
        "CREATE TABLE {table} (id int NOT NULL PRIMARY KEY, name nvarchar(50) NULL, amount int NULL DEFAULT 9);"
    );
    let setup = connection
        .exec_sql_batch(create)
        .await
        .map_err(|error| format!("setup: {error}"));
    let result = async {
        setup.map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;
        let mut bulk = connection.new_bulk_load(
            &table,
            BulkLoadOptions {
                check_constraints: true,
                keep_nulls: true,
                ..Default::default()
            },
        )?;
        bulk.add_typed_column(
            "id",
            tedious_rs::data_type::type_by_name("Int").ok_or("missing Int type")?,
            Default::default(),
            false,
        )?;
        bulk.add_typed_column(
            "name",
            tedious_rs::data_type::type_by_name("NVarChar").ok_or("missing NVarChar type")?,
            tedious_rs::parameter::ParameterOptions {
                length: Some(50),
                ..Default::default()
            },
            true,
        )?;
        bulk.add_typed_column(
            "amount",
            tedious_rs::data_type::type_by_name("Int").ok_or("missing Int type")?,
            Default::default(),
            true,
        )?;
        let tokens = connection
            .exec_bulk_load_values(
                &mut bulk,
                vec![
                    vec![
                        Value::Integer(1),
                        Value::Text("um".into()),
                        Value::Integer(4),
                    ],
                    vec![Value::Integer(2), Value::Null, Value::Null],
                ],
            )
            .await
            .map_err(|error| format!("bulk: {error}"))?;
        assert!(
            tokens
                .iter()
                .any(|token| matches!(token, StreamToken::Control(Token::Done(_))))
        );
        let selected = connection
            .exec_sql_batch(format!("SELECT id, name, amount FROM {table} ORDER BY id;"))
            .await?;
        let rows = selected
            .iter()
            .filter_map(|token| match token {
                StreamToken::Row(row) | StreamToken::NbcRow(row) => Some(row),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].columns[0].value, Value::Integer(1));
        assert_eq!(rows[0].columns[1].value, Value::Text("um".into()));
        assert_eq!(rows[0].columns[2].value, Value::Integer(4));
        assert_eq!(rows[1].columns[0].value, Value::Integer(2));
        assert_eq!(rows[1].columns[1].value, Value::Null);
        assert_eq!(rows[1].columns[2].value, Value::Null);
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    let cleanup = connection
        .exec_sql_batch(format!(
            "IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table};"
        ))
        .await
        .map_err(|error| format!("cleanup: {error}"));
    connection.close().await;
    let result_error = result.err();
    match (result_error, cleanup.err()) {
        (Some(error), _) => Err(error),
        (None, Some(error)) => Err(error.into()),
        (None, None) => Ok(()),
    }
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and the local SQL Authentication endpoint"]
async fn sql_auth_bulk_load_validates_order_clustered_key_and_triggers()
-> Result<(), Box<dyn std::error::Error>> {
    if !crate::common::integration_enabled() {
        return Ok(());
    }
    let table_name = crate::common::sql_auth_object_name("bulk_order");
    let audit_name = crate::common::sql_auth_object_name("bulk_audit");
    let trigger_name = crate::common::sql_auth_object_name("bulk_trigger");
    let table = format!("dbo.{table_name}");
    let audit = format!("dbo.{audit_name}");
    let trigger = format!("dbo.{trigger_name}");
    let cleanup = format!(
        "IF OBJECT_ID(N'{trigger}', N'TR') IS NOT NULL DROP TRIGGER {trigger}; IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table}; IF OBJECT_ID(N'{audit}', N'U') IS NOT NULL DROP TABLE {audit};"
    );
    crate::common::with_sql_connection_cleanup(cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!(
                    "CREATE TABLE {table} (id int NOT NULL, name nvarchar(100) NULL); CREATE CLUSTERED INDEX IX_{table_name} ON {table}(id); CREATE TABLE {audit} (id int NOT NULL);"
                ))
                .await?;
            connection
                .exec_sql_batch(format!(
                    "CREATE TRIGGER {trigger} ON {table} AFTER INSERT AS INSERT INTO {audit}(id) SELECT id FROM inserted;"
                ))
                .await?;

            let mut ordered = connection.new_bulk_load(
                &table,
                BulkLoadOptions {
                    fire_triggers: true,
                    check_constraints: true,
                    order: vec![("id".to_owned(), "ASC".to_owned())],
                    ..Default::default()
                },
            )?;
            add_id_column(&mut ordered)?;
            add_name_column(&mut ordered)?;
            let ordered_tokens = connection
                .exec_bulk_load_values(
                    &mut ordered,
                    vec![
                        vec![Value::Integer(1), Value::Text("primeiro".to_owned())],
                        vec![Value::Integer(2), Value::Text("segundo".to_owned())],
                        vec![Value::Integer(3), Value::Text("terceiro".to_owned())],
                    ],
                )
                .await?;
            assert!(ordered_tokens.iter().any(|token| {
                matches!(token, StreamToken::Control(Token::Done(done)) if done.row_count == Some(3))
            }));

            let audit_rows = connection
                .exec_sql_batch(format!("SELECT COUNT(*) AS total FROM {audit};"))
                .await?;
            assert!(audit_rows.iter().any(|token| {
                matches!(token, StreamToken::Row(row) | StreamToken::NbcRow(row) if row.columns.first().is_some_and(|column| column.value == Value::Integer(3)))
            }));

            let mut invalid_order = connection.new_bulk_load(
                &table,
                BulkLoadOptions {
                    order: vec![("id".to_owned(), "ASC".to_owned())],
                    ..Default::default()
                },
            )?;
            add_id_column(&mut invalid_order)?;
            add_name_column(&mut invalid_order)?;
            let invalid_result = connection
                .exec_bulk_load_values(
                    &mut invalid_order,
                    vec![
                        vec![Value::Integer(9), Value::Text("nove".to_owned())],
                        vec![Value::Integer(8), Value::Text("oito".to_owned())],
                    ],
                )
                .await;
            assert!(invalid_result.is_err());

            let reuse = connection.exec_sql_batch(format!(
                "INSERT INTO {table}(id, name) VALUES (10, N'reuso'); SELECT COUNT(*) AS total FROM {table} WHERE id = 10;"
            )).await?;
            assert!(reuse.iter().any(|token| {
                matches!(token, StreamToken::Row(row) | StreamToken::NbcRow(row) if row.columns.first().is_some_and(|column| column.value == Value::Integer(1)))
            }));
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and the local SQL Authentication endpoint"]
async fn sql_auth_bulk_load_reuses_connection_after_short_producer_row()
-> Result<(), Box<dyn std::error::Error>> {
    if !crate::common::integration_enabled() {
        return Ok(());
    }
    let table_name = crate::common::sql_auth_object_name("bulk_producer");
    let table = format!("dbo.{table_name}");
    let cleanup = format!("IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table};");
    crate::common::with_sql_connection_cleanup(cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!(
                    "CREATE TABLE {table} (id int NOT NULL PRIMARY KEY, name nvarchar(100) NULL);"
                ))
                .await?;
            let mut bulk = connection.new_bulk_load(&table, Default::default())?;
            add_id_column(&mut bulk)?;
            add_name_column(&mut bulk)?;
            let producer_error = bulk.add_value_row(vec![Value::Integer(2)]);
            assert!(producer_error.is_ok());
            assert_eq!(bulk.rows.len(), 1);
            assert_eq!(bulk.rows[0].values.len(), 2);
            let selected = connection
                .exec_sql_batch(format!(
                    "INSERT INTO {table}(id, name) VALUES (3, N'após erro'); SELECT id, name FROM {table} ORDER BY id;"
                ))
                .await?;
            let rows = selected
                .iter()
                .filter_map(|token| match token {
                    StreamToken::Row(row) | StreamToken::NbcRow(row) => Some(row),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].columns[0].value, Value::Integer(3));
            assert_eq!(rows[0].columns[1].value, Value::Text("após erro".to_owned()));
            Ok(())
        })
    })
    .await
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and the local SQL Authentication endpoint"]
async fn sql_auth_streaming_bulk_load_timeout_cancels_payload_and_keeps_connection_logged_in()
-> Result<(), Box<dyn std::error::Error>> {
    if !crate::common::integration_enabled() {
        return Ok(());
    }
    let table_name = crate::common::sql_auth_object_name("bulk_timeout");
    let table = format!("dbo.{table_name}");
    let cleanup = format!("IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table};");
    crate::common::with_sql_connection_cleanup(cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!("CREATE TABLE {table} (id int NOT NULL);"))
                .await?;
            let mut bulk = connection.new_bulk_load(&table, Default::default())?;
            bulk.set_timeout(Some(Duration::from_millis(100)));
            add_id_column(&mut bulk)?;
            let rows = stream::unfold(0usize, |index| async move {
                match index {
                    0 => Some((vec![Value::Integer(1)], 1)),
                    1 => {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        Some((vec![Value::Integer(2)], 2))
                    }
                    _ => None,
                }
            });
            let error = connection
                .exec_bulk_load_value_stream(&mut bulk, rows)
                .await
                .unwrap_err();
            assert_eq!(error.code.as_deref(), Some("ETIMEOUT"));
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
