use tedious_rs::Connection;
use tedious_rs::parameter::{ParameterOptions, SqlParameterType};
use tedious_rs::payloads::{TvpPayload, TvpTypedColumnDefinition};
use tedious_rs::request::Request;
use tedious_rs::value_codec::Value;

fn tvp_columns() -> [TvpTypedColumnDefinition; 2] {
    [
        TvpTypedColumnDefinition {
            name: "id".into(),
            parameter_type: SqlParameterType::Int,
            options: ParameterOptions::default(),
        },
        TvpTypedColumnDefinition {
            name: "name".into(),
            parameter_type: SqlParameterType::NVarChar,
            options: ParameterOptions {
                length: Some(100),
                ..Default::default()
            },
        },
    ]
}

fn add_tvp_parameter(
    request: &mut Request,
    payload: TvpPayload,
) -> Result<(), Box<dyn std::error::Error>> {
    request.add_typed_parameter(
        "items",
        SqlParameterType::Tvp,
        Value::Tvp(Box::new(payload)),
        ParameterOptions::default(),
    )?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and the local SQL Authentication endpoint"]
async fn sql_auth_tvp_round_trip_uses_sql_auth_and_cleans_objects()
-> Result<(), Box<dyn std::error::Error>> {
    if !crate::common::integration_enabled() {
        return Ok(());
    }
    let mut config = crate::common::sql_config()?;
    config.row_collection_on_done = true;
    config.row_collection_on_request_completion = true;
    let mut connection = Connection::new(config);
    connection.connect().await?;
    let type_name = crate::common::sql_auth_object_name("tvp_type");
    let procedure_name = crate::common::sql_auth_object_name("tvp_proc");
    let qualified_type_name = format!("dbo.{type_name}");
    let qualified_procedure_name = format!("dbo.{procedure_name}");
    let create_type = format!(
        "CREATE TYPE {qualified_type_name} AS TABLE (id int NOT NULL, name nvarchar(50) NULL);"
    );
    let create_procedure = format!(
        "CREATE PROCEDURE {qualified_procedure_name} @items {qualified_type_name} READONLY AS SELECT id, name FROM @items ORDER BY id;"
    );
    let setup_result = async {
        connection.exec_sql_batch(create_type).await?;
        connection.exec_sql_batch(create_procedure).await?;
        Ok::<(), tedious_rs::errors::ConnectionError>(())
    }
    .await;
    let result = async {
        setup_result?;
        let columns = [
            TvpTypedColumnDefinition {
                name: "id".into(),
                parameter_type: SqlParameterType::Int,
                options: ParameterOptions::default(),
            },
            TvpTypedColumnDefinition {
                name: "name".into(),
                parameter_type: SqlParameterType::NVarChar,
                options: ParameterOptions {
                    length: Some(50),
                    ..Default::default()
                },
            },
        ];
        let payload = TvpPayload::from_typed_values(
            "",
            "dbo",
            &type_name,
            &columns,
            &[
                vec![Value::Integer(1), Value::Text("um".into())],
                vec![Value::Integer(2), Value::Null],
            ],
            None,
        )?;
        let mut request = Request::new(qualified_procedure_name.clone(), |_| {});
        request.add_typed_parameter(
            "items",
            SqlParameterType::Tvp,
            Value::Tvp(Box::new(payload)),
            ParameterOptions::default(),
        )?;
        connection.call_procedure(&mut request).await?;
        assert_eq!(request.rows.len(), 2);
        assert_eq!(request.rows[0].columns[0].value, Value::Integer(1));
        assert_eq!(request.rows[0].columns[1].value, Value::Text("um".into()));
        assert_eq!(request.rows[1].columns[0].value, Value::Integer(2));
        assert_eq!(request.rows[1].columns[1].value, Value::Null);
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    let cleanup = connection
        .exec_sql_batch(format!(
            "IF OBJECT_ID(N'{qualified_procedure_name}', N'P') IS NOT NULL DROP PROCEDURE {qualified_procedure_name}; IF TYPE_ID(N'{qualified_type_name}') IS NOT NULL DROP TYPE {qualified_type_name};"
        ))
        .await;
    connection.close().await;
    result.and(cleanup.map(|_| ()).map_err(|error| error.into()))
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and the local SQL Authentication endpoint"]
async fn sql_auth_tvp_supports_empty_unicode_and_parameterized_rpc()
-> Result<(), Box<dyn std::error::Error>> {
    if !crate::common::integration_enabled() {
        return Ok(());
    }
    let type_name = crate::common::sql_auth_object_name("tvp_advanced_type");
    let procedure_name = crate::common::sql_auth_object_name("tvp_advanced_proc");
    let qualified_type_name = format!("dbo.{type_name}");
    let qualified_procedure_name = format!("dbo.{procedure_name}");
    let cleanup = format!(
        "IF OBJECT_ID(N'{qualified_procedure_name}', N'P') IS NOT NULL DROP PROCEDURE {qualified_procedure_name}; IF TYPE_ID(N'{qualified_type_name}') IS NOT NULL DROP TYPE {qualified_type_name};"
    );
    let mut config = crate::common::sql_config()?;
    config.row_collection_on_done = true;
    config.row_collection_on_request_completion = true;
    crate::common::with_sql_connection_cleanup_with_config(config, cleanup, move |connection| {
        Box::pin(async move {
            connection
                .exec_sql_batch(format!(
                    "CREATE TYPE {qualified_type_name} AS TABLE (id int NOT NULL, name nvarchar(100) NULL);"
                ))
                .await?;
            connection
                .exec_sql_batch(format!(
                    "CREATE PROCEDURE {qualified_procedure_name} @items {qualified_type_name} READONLY AS SELECT id, name FROM @items ORDER BY id;"
                ))
                .await?;
            let columns = tvp_columns();
            let unicode = TvpPayload::from_typed_values(
                "",
                "dbo",
                &type_name,
                &columns,
                &[
                    vec![Value::Integer(7), Value::Text("ação 東京 😀".to_owned())],
                    vec![Value::Integer(8), Value::Null],
                ],
                None,
            )?;
            let mut unicode_request = Request::new(qualified_procedure_name.clone(), |_| {});
            add_tvp_parameter(
                &mut unicode_request,
                unicode,
            )?;
            connection.call_procedure(&mut unicode_request).await?;
            assert_eq!(unicode_request.rows.len(), 2);
            assert_eq!(unicode_request.rows[0].columns[0].value, Value::Integer(7));
            assert_eq!(
                unicode_request.rows[0].columns[1].value,
                Value::Text("ação 東京 😀".to_owned())
            );
            assert_eq!(unicode_request.rows[1].columns[1].value, Value::Null);

            let empty = TvpPayload::from_typed_values(
                "",
                "dbo",
                &type_name,
                &columns,
                &[],
                None,
            )?;
            let mut empty_request = Request::new(qualified_procedure_name.clone(), |_| {});
            add_tvp_parameter(&mut empty_request, empty)?;
            connection.call_procedure(&mut empty_request).await?;
            assert!(empty_request.rows.is_empty());

            let mut statement = Request::new("SELECT @label AS label", |_| {});
            statement.add_typed_parameter(
                "label",
                SqlParameterType::NVarChar,
                Value::Text("parâmetro 日本語".to_owned()),
                ParameterOptions {
                    length: Some(100),
                    ..Default::default()
                },
            )?;
            connection.exec_sql(&mut statement).await?;
            assert!(statement.rows.iter().any(|row| {
                row.columns.first().is_some_and(|column| {
                    column.value == Value::Text("parâmetro 日本語".to_owned())
                })
            }));

            let malformed = TvpPayload::from_typed_values(
                "",
                "dbo",
                &type_name,
                &columns,
                &[vec![Value::Integer(9)]],
                None,
            );
            assert!(malformed.is_ok());
            let valid_after_error = TvpPayload::from_typed_values(
                "",
                "dbo",
                &type_name,
                &columns,
                &[vec![Value::Integer(10), Value::Text("reuso".to_owned())]],
                None,
            )?;
            let mut reuse_request = Request::new(qualified_procedure_name.clone(), |_| {});
            add_tvp_parameter(
                &mut reuse_request,
                valid_after_error,
            )?;
            connection.call_procedure(&mut reuse_request).await?;
            assert_eq!(reuse_request.rows.len(), 1);
            assert_eq!(reuse_request.rows[0].columns[0].value, Value::Integer(10));
            Ok(())
        })
    })
    .await
}
