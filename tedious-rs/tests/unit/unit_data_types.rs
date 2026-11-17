use tedious_rs::data_type::{TYPES, type_by_id, type_by_name};
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::value_codec::Value;

#[test]
fn registry_contains_every_source_data_type_with_wire_identity() {
    assert_eq!(TYPES.len(), 39);
    for data_type in TYPES {
        assert!(!data_type.type_name.is_empty());
        assert!(!data_type.name.is_empty());
        assert_eq!(type_by_id(data_type.id), Some(data_type));
        assert_eq!(type_by_name(data_type.name), Some(data_type));
        if data_type.name != "Null" {
            assert!(data_type.parameter_type.is_some());
        }
    }
}

#[test]
fn source_data_type_modules_resolve_to_the_registry_entries() {
    assert_eq!(tedious_rs::data_types::int::data_type().name, "Int");
    assert_eq!(
        tedious_rs::data_types::nvarchar::data_type().name,
        "NVarChar"
    );
    assert_eq!(
        tedious_rs::data_types::sql_variant::data_type().name,
        "Variant"
    );
    assert_eq!(tedious_rs::data_types::tvp::data_type().id, 0xF3);
    assert_eq!(tedious_rs::data_types::udt::data_type().id, 0xF0);
}

#[test]
fn every_source_data_type_module_exposes_executable_parameter_contracts() {
    let names = [
        tedious_rs::data_types::bigint::data_type().name,
        tedious_rs::data_types::binary::data_type().name,
        tedious_rs::data_types::bit::data_type().name,
        tedious_rs::data_types::bitn::data_type().name,
        tedious_rs::data_types::char::data_type().name,
        tedious_rs::data_types::date::data_type().name,
        tedious_rs::data_types::datetime::data_type().name,
        tedious_rs::data_types::datetime2::data_type().name,
        tedious_rs::data_types::datetimen::data_type().name,
        tedious_rs::data_types::datetimeoffset::data_type().name,
        tedious_rs::data_types::decimal::data_type().name,
        tedious_rs::data_types::decimaln::data_type().name,
        tedious_rs::data_types::float::data_type().name,
        tedious_rs::data_types::floatn::data_type().name,
        tedious_rs::data_types::image::data_type().name,
        tedious_rs::data_types::int::data_type().name,
        tedious_rs::data_types::intn::data_type().name,
        tedious_rs::data_types::money::data_type().name,
        tedious_rs::data_types::moneyn::data_type().name,
        tedious_rs::data_types::nchar::data_type().name,
        tedious_rs::data_types::ntext::data_type().name,
        tedious_rs::data_types::null::data_type().name,
        tedious_rs::data_types::numeric::data_type().name,
        tedious_rs::data_types::numericn::data_type().name,
        tedious_rs::data_types::nvarchar::data_type().name,
        tedious_rs::data_types::real::data_type().name,
        tedious_rs::data_types::smalldatetime::data_type().name,
        tedious_rs::data_types::smallint::data_type().name,
        tedious_rs::data_types::smallmoney::data_type().name,
        tedious_rs::data_types::sql_variant::data_type().name,
        tedious_rs::data_types::text::data_type().name,
        tedious_rs::data_types::time::data_type().name,
        tedious_rs::data_types::tinyint::data_type().name,
        tedious_rs::data_types::tvp::data_type().name,
        tedious_rs::data_types::udt::data_type().name,
        tedious_rs::data_types::uniqueidentifier::data_type().name,
        tedious_rs::data_types::varbinary::data_type().name,
        tedious_rs::data_types::varchar::data_type().name,
        tedious_rs::data_types::xml::data_type().name,
    ];
    assert_eq!(names.len(), 39);
    assert!(names.iter().all(|name| !name.is_empty()));
}

#[test]
fn type_module_contract_delegates_metadata_length_data_and_validation() {
    let options = ParameterOptions {
        output: false,
        length: Some(8),
        precision: Some(9),
        scale: Some(2),
        collation: None,
    };
    let value = Value::Integer(42);
    assert_eq!(
        tedious_rs::data_types::int::declaration(&value, &options, false).unwrap(),
        "int"
    );
    assert_eq!(
        tedious_rs::data_types::int::generate_type_info(&value, &options).unwrap(),
        vec![0x26, 4]
    );
    assert_eq!(
        tedious_rs::data_types::int::generate_parameter_length(&value, &options).unwrap(),
        vec![4]
    );
    assert_eq!(
        tedious_rs::data_types::int::generate_parameter_data(&value, &options).unwrap(),
        42_i32.to_le_bytes()
    );
    assert!(tedious_rs::data_types::int::validate(&value, &options).is_ok());
    assert!(
        tedious_rs::data_types::int::validate(&Value::Text("bad".to_owned()), &options).is_err()
    );

    let text = Value::Text("á".to_owned());
    assert_eq!(
        tedious_rs::data_types::nvarchar::generate_parameter_data(&text, &options).unwrap(),
        vec![0xE1, 0]
    );
    assert_eq!(
        tedious_rs::data_types::decimal::generate_parameter_data(
            &Value::Decimal {
                unscaled: 1234,
                scale: 2
            },
            &options
        )
        .unwrap(),
        vec![1, 0xD2, 4, 0, 0]
    );
}

#[test]
fn declarations_preserve_source_sql_names_and_options() {
    let options = ParameterOptions {
        output: false,
        length: Some(12),
        precision: Some(19),
        scale: Some(4),
        collation: None,
    };
    assert_eq!(
        type_by_name("NVarChar")
            .unwrap()
            .declaration(&Value::Text("hello".to_owned()), &options, false)
            .unwrap(),
        "nvarchar(12)"
    );
    assert_eq!(
        type_by_name("Decimal")
            .unwrap()
            .declaration(
                &Value::Decimal {
                    unscaled: 12345,
                    scale: 4
                },
                &options,
                false
            )
            .unwrap(),
        "decimal(19, 4)"
    );
    assert_eq!(
        type_by_name("DateTime2")
            .unwrap()
            .declaration(&Value::Null, &options, false)
            .unwrap(),
        "datetime2(4)"
    );
}

#[test]
fn nullable_and_structural_declarations_are_executable() {
    let options = ParameterOptions::default();
    let value = Value::Integer(1);
    for name in [
        "BitN",
        "DateTimeN",
        "DecimalN",
        "NumericN",
        "FloatN",
        "MoneyN",
    ] {
        assert!(
            !type_by_name(name)
                .unwrap()
                .declaration(&value, &options, false)
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        type_by_name("Null")
            .unwrap()
            .declaration(&Value::Null, &options, false)
            .unwrap(),
        "NULL"
    );
    let udt = Value::Udt {
        database: "db".to_owned(),
        schema: "dbo".to_owned(),
        type_name: "point".to_owned(),
        data: vec![],
    };
    assert_eq!(
        type_by_name("UDT")
            .unwrap()
            .declaration(&udt, &options, false)
            .unwrap(),
        "db.dbo.point"
    );
    assert_eq!(
        type_by_name("Variant")
            .unwrap()
            .declaration(&value, &options, false)
            .unwrap(),
        "sql_variant"
    );
}

#[test]
fn decimal_defaults_match_node_resolution() {
    let data_type = type_by_name("Decimal").unwrap();
    let value = Value::Decimal {
        unscaled: 123,
        scale: 0,
    };
    assert_eq!(
        data_type
            .declaration(&value, &ParameterOptions::default(), false)
            .unwrap(),
        "decimal(18, 0)"
    );
    assert_eq!(
        data_type
            .encode(&value, &ParameterOptions::default())
            .unwrap()
            .length,
        vec![9]
    );
}

#[test]
fn numeric_declaration_uses_numeric_sql_type() {
    let data_type = type_by_name("Numeric").unwrap();
    let value = Value::Decimal {
        unscaled: 123,
        scale: 0,
    };
    assert_eq!(
        data_type
            .declaration(&value, &ParameterOptions::default(), false)
            .unwrap(),
        "numeric(18, 0)"
    );
}

#[test]
fn binary_declaration_preserves_explicit_lengths_above_maximum() {
    let options = ParameterOptions {
        length: Some(9001),
        ..Default::default()
    };
    let value = Value::Binary(Vec::new());
    assert_eq!(
        type_by_name("Binary")
            .unwrap()
            .declaration(&value, &options, false)
            .unwrap(),
        "binary(9001)"
    );
    assert_eq!(
        type_by_name("VarBinary")
            .unwrap()
            .declaration(&value, &options, false)
            .unwrap(),
        "varbinary(max)"
    );
}

#[test]
fn empty_binary_and_text_lengths_match_node_resolution() {
    let value = Value::Binary(Vec::new());
    let text = Value::Text(String::new());
    let options = ParameterOptions::default();

    for name in ["Binary", "VarBinary"] {
        let data_type = type_by_name(name).unwrap();
        assert_eq!(data_type.resolve_length(&value, false), 0);
        assert_eq!(
            data_type.declaration(&value, &options, false).unwrap(),
            format!("{}(1)", name.to_lowercase())
        );
    }

    let data_type = type_by_name("Text").unwrap();
    assert_eq!(
        data_type.resolve_length_with_options(&text, false, &options),
        0
    );
}

#[test]
fn null_legacy_lob_lengths_match_node_resolution() {
    for name in ["Text", "NText", "Image"] {
        assert_eq!(
            type_by_name(name)
                .unwrap()
                .resolve_length(&Value::Null, false),
            -1,
            "null {name} length must use the Node sentinel"
        );
    }
}

#[test]
fn null_legacy_lob_type_info_uses_the_negative_one_length() {
    let options = ParameterOptions::default();
    let expected = (-1_i32).to_le_bytes();
    for name in ["Text", "NText", "Image"] {
        assert_eq!(
            type_by_name(name)
                .unwrap()
                .encode(&Value::Null, &options)
                .unwrap()
                .type_info[1..5],
            expected,
            "null {name} type info must encode the Node sentinel"
        );
    }
}

#[test]
fn public_resolution_helpers_match_node_value_sensitive_defaults() {
    let decimal = type_by_name("Decimal").unwrap();
    let temporal = type_by_name("Time").unwrap();
    let defaults = ParameterOptions::default();

    assert_eq!(
        decimal.resolve_precision_for_value(&Value::Null, &defaults),
        1
    );
    assert_eq!(
        decimal.resolve_precision_for_value(&Value::Integer(1), &defaults),
        18
    );
    assert_eq!(decimal.resolve_scale_for_value(&Value::Null, &defaults), 0);
    assert_eq!(
        temporal.resolve_temporal_scale_for_value(&Value::Null, &defaults),
        0
    );
    assert_eq!(
        temporal.resolve_temporal_scale_for_value(&Value::Integer(1), &defaults),
        7
    );

    let explicit_zero = ParameterOptions {
        precision: Some(0),
        scale: Some(0),
        ..defaults
    };
    assert_eq!(
        decimal.resolve_precision_for_value(&Value::Integer(1), &explicit_zero),
        0
    );
    assert_eq!(
        temporal.resolve_temporal_scale_for_value(&Value::Integer(1), &explicit_zero),
        0
    );
}

#[test]
fn non_unicode_parameters_use_declared_collation_codepage() {
    let options = ParameterOptions {
        length: Some(1),
        collation: Some(tedious_rs::collation::Collation::new(0x0419, 0, 0, 0)),
        ..Default::default()
    };
    let encoded = tedious_rs::parameter::SqlParameterType::VarChar
        .encode(&Value::Text("Ж".to_owned()), &options)
        .unwrap();
    assert_eq!(encoded.data, vec![0xC6]);
    assert_eq!(
        &encoded.type_info[3..8],
        &options.collation.unwrap().to_buffer()
    );
}

#[test]
fn structured_types_fail_without_required_server_side_names() {
    let options = ParameterOptions::default();
    let tvp = type_by_name("TVP")
        .unwrap()
        .declaration(&Value::Null, &options, false)
        .unwrap_err();
    let udt = type_by_name("UDT")
        .unwrap()
        .declaration(&Value::Null, &options, false)
        .unwrap_err();
    assert!(tvp.0.contains("named table type"));
    assert!(udt.0.contains("type metadata"));
}

#[test]
fn udt_declaration_uses_named_server_type() {
    let value = Value::Udt {
        database: "db".to_owned(),
        schema: "dbo".to_owned(),
        type_name: "point".to_owned(),
        data: vec![1, 2, 3],
    };
    assert_eq!(
        type_by_name("UDT")
            .unwrap()
            .declaration(&value, &ParameterOptions::default(), false)
            .unwrap(),
        "db.dbo.point"
    );
}

#[test]
fn tvp_declaration_matches_node_qualified_readonly_form() {
    let value = Value::Tvp(Box::new(tedious_rs::payloads::TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "items".to_owned(),
        columns: Vec::new(),
        rows: Vec::new(),
    }));
    assert_eq!(
        type_by_name("TVP")
            .unwrap()
            .declaration(&value, &ParameterOptions::default(), false)
            .unwrap(),
        "dbo.items readonly"
    );
}

#[test]
fn zero_options_follow_node_truthiness_and_resolve_defaults() {
    let value = Value::Text("abc".to_owned());
    let options = ParameterOptions {
        length: Some(0),
        ..Default::default()
    };
    assert_eq!(
        type_by_name("NVarChar")
            .unwrap()
            .declaration(&value, &options, false)
            .unwrap(),
        "nvarchar(3)"
    );
    let temporal_options = ParameterOptions {
        scale: Some(0),
        ..Default::default()
    };
    assert_eq!(
        type_by_name("Time")
            .unwrap()
            .declaration(
                &Value::Time {
                    hour: 1,
                    minute: 2,
                    second: 3,
                    nanosecond: 4,
                },
                &temporal_options,
                false,
            )
            .unwrap(),
        "time(0)"
    );
}

#[test]
fn temporal_and_numeric_defaults_follow_node_nullability_rules() {
    let temporal = Value::Time {
        hour: 1,
        minute: 2,
        second: 3,
        nanosecond: 4,
    };
    assert_eq!(
        type_by_name("Time")
            .unwrap()
            .declaration(&temporal, &ParameterOptions::default(), false)
            .unwrap(),
        "time(7)"
    );
    assert_eq!(
        type_by_name("Time")
            .unwrap()
            .declaration(&Value::Null, &ParameterOptions::default(), false)
            .unwrap(),
        "time(0)"
    );
    assert_eq!(
        type_by_name("Decimal")
            .unwrap()
            .declaration(&Value::Null, &ParameterOptions::default(), false)
            .unwrap(),
        "decimal(1, 0)"
    );
}
