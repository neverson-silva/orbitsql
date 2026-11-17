use std::collections::BTreeMap;

use tedious_rs::bulk_load::{BulkLoad, BulkLoadOptions};
use tedious_rs::data_type::type_by_name;
use tedious_rs::value_codec::Value;

#[test]
fn bulk_options_match_insert_bulk_sql_shape_without_duplicate_tablock() {
    let bulk = BulkLoad::new(
        "dbo.target",
        BulkLoadOptions {
            check_constraints: true,
            fire_triggers: true,
            keep_nulls: true,
            lock_table: true,
            order: vec![
                ("second".to_owned(), "DESC".to_owned()),
                ("first".to_owned(), "ASC".to_owned()),
            ],
        },
    )
    .unwrap();
    assert_eq!(
        bulk.get_options_sql(),
        " WITH (CHECK_CONSTRAINTS,FIRE_TRIGGERS,KEEP_NULLS,TABLOCK,ORDER (second DESC, first ASC))"
    );
}

#[test]
fn bulk_order_rejects_non_exact_node_directions() {
    assert!(
        BulkLoad::new(
            "dbo.target",
            BulkLoadOptions {
                order: vec![("id".to_owned(), "asc".to_owned())],
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn bulk_object_rows_follow_declared_columns_and_object_names() {
    let mut bulk = BulkLoad::new("dbo.target", Default::default()).unwrap();
    bulk.add_typed_column(
        "id",
        type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    bulk.add_typed_column_with_object_name(
        "description",
        "label",
        type_by_name("NVarChar").unwrap(),
        tedious_rs::parameter::ParameterOptions {
            length: Some(20),
            ..Default::default()
        },
        true,
    )
    .unwrap();
    let mut row = BTreeMap::new();
    row.insert("label".to_owned(), Value::Text("ok".to_owned()));
    row.insert("id".to_owned(), Value::Integer(9));
    bulk.add_object_row(&row).unwrap();
    assert_eq!(bulk.rows[0].values[0].data, 9_i32.to_le_bytes());
    assert_eq!(bulk.rows[0].values[1].data, vec![b'o', 0, b'k', 0]);
}

#[test]
fn bulk_legacy_lob_nulls_use_the_node_null_pointer_marker() {
    for type_name in ["Text", "NText", "Image"] {
        let mut bulk = BulkLoad::new("dbo.target", Default::default()).unwrap();
        bulk.add_typed_column(
            "value",
            type_by_name(type_name).unwrap(),
            Default::default(),
            true,
        )
        .unwrap();
        bulk.add_value_row(vec![Value::Null]).unwrap();
        assert!(bulk.rows[0].values[0].length.is_empty());
        assert!(
            bulk.payload()
                .row_buffers(&bulk.rows[0])
                .unwrap()
                .iter()
                .any(|buffer| buffer == &[0])
        );
    }
}

#[test]
fn bulk_legacy_lob_empty_values_are_not_null() {
    let mut bulk = BulkLoad::new("dbo.target", Default::default()).unwrap();
    bulk.add_typed_column(
        "value",
        type_by_name("Text").unwrap(),
        Default::default(),
        true,
    )
    .unwrap();
    bulk.add_value_row(vec![Value::Text(String::new())])
        .unwrap();
    assert_eq!(bulk.rows[0].values[0].length, 0_u32.to_le_bytes().to_vec());
    assert_eq!(
        bulk.payload().row_buffers(&bulk.rows[0]).unwrap()[1].len(),
        24
    );
}

#[test]
fn bulk_execution_validation_precedes_execution_state_transition() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    assert!(bulk.begin_execution().is_ok());
    assert!(bulk.execution_started);
}

#[test]
fn bulk_execution_does_not_prevalidate_stored_rows() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "id",
        type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    bulk.rows
        .push(tedious_rs::payloads::BulkLoadRow { values: Vec::new() });
    assert!(bulk.begin_execution().is_ok());
    assert!(bulk.execution_started);
}

#[test]
fn bulk_nullable_metadata_does_not_reject_null_values() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "id",
        type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    assert!(bulk.add_value_row(vec![Value::Null]).is_ok());
}

#[test]
fn bulk_column_metadata_names_are_not_validated_before_execution() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    assert!(
        bulk.add_column(tedious_rs::bulk_load::BulkLoadColumnDefinition {
            name: String::new(),
            obj_name: String::new(),
            declaration: String::new(),
            type_info: Vec::new(),
            nullable: true,
            legacy_lob: false,
        })
        .is_ok()
    );
}

#[test]
fn bulk_main_metadata_path_can_encode_nullable_unknown_columns() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column(
        "id",
        type_by_name("Int").unwrap(),
        Default::default(),
        false,
    )
    .unwrap();
    bulk.set_nullable_unknown_columns([0]).unwrap();
    assert_eq!(&bulk.get_col_metadata().unwrap()[7..9], &[0x04, 0x80]);
}

#[test]
fn tvp_typed_builder_matches_node_empty_wire_column_names_and_unicode_values() {
    let columns = [
        tedious_rs::payloads::TvpTypedColumnDefinition {
            name: "identifier".to_owned(),
            parameter_type: tedious_rs::parameter::SqlParameterType::Int,
            options: Default::default(),
        },
        tedious_rs::payloads::TvpTypedColumnDefinition {
            name: "display".to_owned(),
            parameter_type: tedious_rs::parameter::SqlParameterType::NVarChar,
            options: tedious_rs::parameter::ParameterOptions {
                length: Some(20),
                ..Default::default()
            },
        },
    ];
    let payload = tedious_rs::payloads::TvpPayload::from_typed_values(
        "",
        "dbo",
        "items_type",
        &columns,
        &[vec![Value::Integer(3), Value::Text("ação".to_owned())]],
        None,
    )
    .unwrap();
    assert!(payload.columns.iter().all(|column| column.name.is_empty()));
    assert!(payload.data().is_ok());
    assert_eq!(payload.rows[0].values[1].data.len(), 8);
}

#[test]
fn tvp_validation_accepts_node_shaped_rows_and_type_metadata() {
    let rows_without_columns = tedious_rs::payloads::TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items_type".to_owned(),
        columns: Vec::new(),
        rows: vec![tedious_rs::payloads::TvpRow { values: Vec::new() }],
    };
    assert!(rows_without_columns.validate().is_ok());
    let unknown_type = tedious_rs::payloads::TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items_type".to_owned(),
        columns: vec![tedious_rs::payloads::TvpColumnDefinition {
            name: String::new(),
            type_info: vec![0xFE],
        }],
        rows: Vec::new(),
    };
    assert!(unknown_type.validate().is_ok());
}
