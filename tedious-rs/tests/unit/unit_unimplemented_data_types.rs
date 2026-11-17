use tedious_rs::collation::Collation;
use tedious_rs::data_type::type_by_name;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::value_codec::Value;

#[test]
fn nullable_wire_aliases_encode_null_values() {
    for (name, id) in [
        ("IntN", 0x26),
        ("BitN", 0x68),
        ("FloatN", 0x6D),
        ("MoneyN", 0x6E),
        ("DateTimeN", 0x6F),
        ("DecimalN", 0x6A),
        ("NumericN", 0x6C),
    ] {
        let encoded = type_by_name(name)
            .unwrap()
            .encode(&Value::Null, &ParameterOptions::default())
            .unwrap();
        assert_eq!(encoded.type_info[0], id);
        assert_eq!(encoded.length, [0]);
        assert!(encoded.data.is_empty());
    }
}

#[test]
fn null_type_has_a_wire_representation() {
    let encoded = type_by_name("Null")
        .unwrap()
        .encode(&Value::Null, &ParameterOptions::default())
        .unwrap();
    assert_eq!(encoded.type_info, [0x1F]);
    assert!(encoded.length.is_empty());
    assert!(encoded.data.is_empty());
}

#[test]
fn xml_and_udt_encode_plp_payloads() {
    let xml = type_by_name("Xml")
        .unwrap()
        .encode(
            &Value::Text("<x>é</x>".to_owned()),
            &ParameterOptions::default(),
        )
        .unwrap();
    assert_eq!(xml.type_info, [0xF1, 0]);
    assert_eq!(xml.length, [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(!xml.data.is_empty());
    let udt_value = Value::Udt {
        database: "db".to_owned(),
        schema: "dbo".to_owned(),
        type_name: "point".to_owned(),
        data: vec![1, 2, 3],
    };
    let udt = type_by_name("UDT")
        .unwrap()
        .encode(&udt_value, &ParameterOptions::default())
        .unwrap();
    assert_eq!(udt.type_info[0], 0xF0);
    assert!(!udt.data.is_empty());
    assert_eq!(udt.length, [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
}

#[test]
fn sql_variant_encodes_supported_values_without_f64() {
    let datatype = type_by_name("Variant").unwrap();
    for value in [
        Value::Integer(2_147_483_647),
        Value::Text("variant".to_owned()),
        Value::Decimal {
            unscaled: 12345678901234567890123456789012345678,
            scale: 2,
        },
    ] {
        let encoded = datatype
            .encode(&value, &ParameterOptions::default())
            .unwrap();
        assert_eq!(encoded.type_info, [0x62]);
        assert_eq!(encoded.length.len(), 4);
        assert!(!encoded.data.is_empty());
    }
    let collation = Collation::new(0x0409, 0, 0, 0);
    assert_eq!(
        tedious_rs::value_codec::encode_variant(&Value::Text("x".to_owned()), &collation)
            .unwrap()
            .type_info,
        [0x62]
    );
}
