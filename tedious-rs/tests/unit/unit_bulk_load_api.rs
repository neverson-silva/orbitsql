use futures_util::stream;
use std::collections::BTreeMap;
use std::time::Duration;
use tedious_rs::bulk_load::{BulkLoad, BulkLoadColumnDefinition, BulkLoadEvent, BulkLoadOptions};
use tedious_rs::connection::{Connection, ConnectionConfig};
use tedious_rs::value_codec::{Value, encode_int};

#[test]
fn builds_bulk_insert_and_table_sql_with_options() {
    let mut bulk = BulkLoad::new(
        "dbo.items",
        BulkLoadOptions {
            lock_table: true,
            order: vec![("id".to_owned(), "ASC".to_owned())],
            ..BulkLoadOptions::default()
        },
    )
    .unwrap();
    bulk.add_column(BulkLoadColumnDefinition {
        name: "id".to_owned(),
        obj_name: "id".to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable: false,
        legacy_lob: false,
    })
    .unwrap();
    bulk.add_row(vec![encode_int(&Value::Integer(1)).unwrap()])
        .unwrap();
    assert_eq!(
        bulk.get_bulk_insert_sql(),
        "insert bulk dbo.items([id] int) WITH (TABLOCK,ORDER (id ASC))"
    );
    assert_eq!(bulk.get_options_sql(), " WITH (TABLOCK,ORDER (id ASC))");
    assert_eq!(
        bulk.get_table_creation_sql(),
        "CREATE TABLE dbo.items(\n[id] int NOT NULL\n)"
    );
}

#[test]
fn add_row_uses_null_for_missing_columns_and_ignores_extra_values() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_column(BulkLoadColumnDefinition {
        name: "id".to_owned(),
        obj_name: "id".to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable: true,
        legacy_lob: false,
    })
    .unwrap();
    bulk.add_column(BulkLoadColumnDefinition {
        name: "name".to_owned(),
        obj_name: "name".to_owned(),
        declaration: "nvarchar(20)".to_owned(),
        type_info: vec![0xE7, 40, 0, 0, 0, 0, 0, 0],
        nullable: true,
        legacy_lob: false,
    })
    .unwrap();

    bulk.add_row(vec![encode_int(&Value::Integer(1)).unwrap()])
        .unwrap();
    assert_eq!(bulk.rows[0].values.len(), 2);
    assert_eq!(bulk.rows[0].values[1].length, vec![0xFF, 0xFF]);
    assert!(bulk.rows[0].values[1].data.is_empty());

    bulk.add_row(vec![
        encode_int(&Value::Integer(2)).unwrap(),
        encode_int(&Value::Integer(3)).unwrap(),
        encode_int(&Value::Integer(4)).unwrap(),
    ])
    .unwrap();
    assert_eq!(bulk.rows[1].values.len(), 2);
    assert_eq!(bulk.rows[1].values[0].data, 2_i32.to_le_bytes());
    assert_eq!(bulk.rows[1].values[1].data, 3_i32.to_le_bytes());
}

#[test]
fn accepts_empty_table_name_and_duplicate_column_names() {
    let mut bulk = BulkLoad::new("", BulkLoadOptions::default()).unwrap();
    let column = || BulkLoadColumnDefinition {
        name: "id".to_owned(),
        obj_name: "id".to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable: true,
        legacy_lob: false,
    };

    assert!(bulk.add_column(column()).is_ok());
    assert!(bulk.add_column(column()).is_ok());
    assert_eq!(bulk.columns.len(), 2);
}

#[test]
fn rejects_late_columns_and_cancelled_execution() {
    let mut bulk = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    bulk.add_column(BulkLoadColumnDefinition {
        name: "id".to_owned(),
        obj_name: "id".to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable: true,
        legacy_lob: false,
    })
    .unwrap();
    bulk.add_row(vec![encode_int(&Value::Integer(1)).unwrap()])
        .unwrap();
    assert!(
        bulk.add_column(BulkLoadColumnDefinition {
            name: "name".to_owned(),
            obj_name: "name".to_owned(),
            declaration: "nvarchar(20)".to_owned(),
            type_info: vec![0xE7, 40, 0, 0, 0, 0, 0, 0],
            nullable: true,
            legacy_lob: false,
        })
        .is_err()
    );
    bulk.cancel();
    bulk.cancel();
    assert!(bulk.canceled);
    bulk.set_timeout(Some(Duration::from_millis(10)));
    assert_eq!(bulk.timeout, Some(Duration::from_millis(10)));
    bulk.set_timeout(Some(Duration::ZERO));
    assert_eq!(bulk.timeout, Some(Duration::ZERO));
}

#[test]
fn rejects_columns_after_a_row_before_execution_state() {
    let mut bulk = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    bulk.add_column(BulkLoadColumnDefinition {
        name: "id".to_owned(),
        obj_name: "id".to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable: true,
        legacy_lob: false,
    })
    .unwrap();
    bulk.add_row(vec![encode_int(&Value::Integer(1)).unwrap()])
        .unwrap();
    bulk.execution_started = true;

    let error = bulk
        .add_column(BulkLoadColumnDefinition {
            name: "name".to_owned(),
            obj_name: "name".to_owned(),
            declaration: "nvarchar(20)".to_owned(),
            type_info: vec![0xE7, 40, 0, 0, 0, 0, 0, 0],
            nullable: true,
            legacy_lob: false,
        })
        .unwrap_err();
    assert_eq!(error, "cannot add bulk-load columns after the first row");
}

#[test]
fn creates_done_token_without_serializing_stored_rows() {
    let mut bulk = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    bulk.add_column(BulkLoadColumnDefinition {
        name: "id".to_owned(),
        obj_name: "id".to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable: true,
        legacy_lob: false,
    })
    .unwrap();
    bulk.rows
        .push(tedious_rs::payloads::BulkLoadRow { values: Vec::new() });

    assert_eq!(
        bulk.create_done_token().unwrap(),
        vec![0xFD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}

#[test]
fn bulk_load_cancel_emits_one_event() {
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&events);
    let mut bulk = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    bulk.on(move |event| seen.lock().unwrap().push(event.clone()));
    bulk.cancel();
    bulk.cancel();
    assert_eq!(*events.lock().unwrap(), vec![BulkLoadEvent::Cancel]);
}

#[test]
fn connection_creates_bulk_load_with_node_style_factory() {
    let connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let bulk = connection
        .new_bulk_load("dbo.items", BulkLoadOptions::default())
        .unwrap();
    assert_eq!(bulk.table, "dbo.items");
}

#[tokio::test]
async fn async_bulk_load_stream_api_preserves_connection_precondition() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut bulk = connection
        .new_bulk_load("dbo.items", BulkLoadOptions::default())
        .unwrap();
    let result = connection
        .exec_bulk_load_value_stream(&mut bulk, stream::once(async { Vec::new() }))
        .await;
    assert_eq!(result.unwrap_err().code.as_deref(), Some("ENOTCONNECTED"));
    assert!(!bulk.execution_started);
}

#[tokio::test]
async fn bulk_load_reports_connection_error_through_completion_callback() {
    let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&observed);
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut bulk =
        BulkLoad::new_with_callback("dbo.items", BulkLoadOptions::default(), move |result| {
            seen.lock().unwrap().push(result)
        })
        .unwrap();
    bulk.add_column(BulkLoadColumnDefinition {
        name: "id".to_owned(),
        obj_name: "id".to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable: false,
        legacy_lob: false,
    })
    .unwrap();
    let error = connection
        .exec_bulk_load_values(&mut bulk, [vec![Value::Integer(1)]])
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert_eq!(observed.lock().unwrap().len(), 1);
    assert_eq!(
        observed.lock().unwrap()[0],
        Err("connection is not logged in".to_owned())
    );
}

#[test]
fn typed_bulk_load_columns_encode_logical_rows() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    bulk.add_typed_column(
        "name",
        tedious_rs::data_type::type_by_name("NVarChar").unwrap(),
        tedious_rs::parameter::ParameterOptions {
            length: Some(20),
            ..Default::default()
        },
        true,
    )
    .unwrap();
    bulk.add_value_row(vec![Value::Integer(7), Value::Text("sete".to_owned())])
        .unwrap();
    assert_eq!(bulk.rows.len(), 1);
    assert_eq!(bulk.rows[0].values[0].data, 7_i32.to_le_bytes());
    assert_eq!(
        bulk.rows[0].values[1].data,
        vec![b's', 0, b'e', 0, b't', 0, b'e', 0]
    );
}

#[test]
fn typed_array_rows_use_null_for_missing_values_and_ignore_extra_values() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        true,
    )
    .unwrap();
    bulk.add_typed_column(
        "name",
        tedious_rs::data_type::type_by_name("NVarChar").unwrap(),
        tedious_rs::parameter::ParameterOptions {
            length: Some(20),
            ..Default::default()
        },
        true,
    )
    .unwrap();

    bulk.add_value_row(vec![Value::Integer(1)]).unwrap();
    assert_eq!(bulk.rows[0].values.len(), 2);
    assert_eq!(bulk.rows[0].values[1].length, vec![0xFF, 0xFF]);

    bulk.add_value_row(vec![
        Value::Integer(2),
        Value::Text("dois".to_owned()),
        Value::Integer(3),
    ])
    .unwrap();
    assert_eq!(bulk.rows[1].values.len(), 2);
    assert_eq!(bulk.rows[1].values[0].data, 2_i32.to_le_bytes());
    assert_eq!(
        bulk.rows[1].values[1].data,
        vec![b'd', 0, b'o', 0, b'i', 0, b's', 0]
    );
}

#[test]
fn typed_bulk_load_uses_node_maximum_length_when_length_is_omitted() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "name",
        tedious_rs::data_type::type_by_name("NVarChar").unwrap(),
        Default::default(),
        true,
    )
    .unwrap();
    assert_eq!(bulk.columns[0].declaration, "nvarchar(4000)");
    assert_eq!(
        bulk.columns[0].type_info,
        vec![0xE7, 0x40, 0x1F, 0, 0, 0, 0, 0]
    );
}

#[test]
fn typed_bulk_load_rejects_null_for_non_nullable_column() {
    let mut bulk = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    assert!(bulk.add_value_row(vec![Value::Null]).is_ok());
}

#[test]
fn object_rows_use_obj_name_and_preserve_declared_column_order() {
    let mut bulk = BulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    bulk.add_typed_column_with_object_name(
        "display_name",
        "name",
        tedious_rs::data_type::type_by_name("NVarChar").unwrap(),
        tedious_rs::parameter::ParameterOptions {
            length: Some(20),
            ..Default::default()
        },
        true,
    )
    .unwrap();
    let mut row = BTreeMap::new();
    row.insert("name".to_owned(), Value::Text("sete".to_owned()));
    row.insert("extra".to_owned(), Value::Text("ignored".to_owned()));
    assert!(bulk.add_object_row(&row).is_ok());

    row.insert("id".to_owned(), Value::Integer(7));
    bulk.add_object_row(&row).unwrap();
    assert_eq!(bulk.rows.len(), 2);
    assert_eq!(bulk.rows[1].values[0].data, 7_i32.to_le_bytes());
    assert_eq!(
        bulk.rows[1].values[1].data,
        vec![b's', 0, b'e', 0, b't', 0, b'e', 0]
    );
}

#[tokio::test]
async fn connection_bulk_load_objects_streams_rows_before_connection_validation() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut bulk = connection
        .new_bulk_load("dbo.items", BulkLoadOptions::default())
        .unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    let mut row = BTreeMap::new();
    row.insert("id".to_owned(), Value::Integer(7));
    let error = connection
        .exec_bulk_load_objects(&mut bulk, [row])
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert!(bulk.rows.is_empty());
}

#[tokio::test]
async fn connection_bulk_load_values_streams_array_rows_before_connection_validation() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "sa", "secret"));
    let mut bulk = connection
        .new_bulk_load("dbo.items", BulkLoadOptions::default())
        .unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    let error = connection
        .exec_bulk_load_values(&mut bulk, [vec![Value::Integer(7)]])
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("ENOTCONNECTED"));
    assert!(bulk.rows.is_empty());
}

#[test]
fn typed_bulk_load_uses_the_connection_collation_for_non_unicode_columns() {
    let mut bulk = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    bulk.set_collation(Some(tedious_rs::collation::Collation::new(0x0419, 0, 0, 0)));
    bulk.add_typed_column(
        "name",
        tedious_rs::data_type::type_by_name("VarChar").unwrap(),
        tedious_rs::parameter::ParameterOptions {
            length: Some(10),
            ..Default::default()
        },
        true,
    )
    .unwrap();
    bulk.add_value_row(vec![Value::Text("Ж".to_owned())])
        .unwrap();
    assert_eq!(bulk.rows[0].values[0].data, vec![0xC6]);
}

#[test]
fn bulk_load_exposes_node_payload_helpers() {
    let mut bulk = BulkLoad::new("items", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "id",
        tedious_rs::data_type::type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    assert_eq!(
        bulk.get_col_metadata().unwrap(),
        bulk.payload().column_metadata().unwrap()
    );
    assert_eq!(bulk.create_done_token().unwrap().len(), 13);
    assert_eq!(bulk.to_string("  "), "  BulkLoad");
}
