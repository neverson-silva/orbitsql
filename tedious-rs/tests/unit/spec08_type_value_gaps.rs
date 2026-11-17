use tedious_rs::collation::Collation;
use tedious_rs::data_type::type_by_name;
use tedious_rs::metadata_parser::Metadata;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::value_codec::Value;
use tedious_rs::value_parser::read_value;

fn options() -> ParameterOptions {
    ParameterOptions::default()
}

fn metadata(name: &str) -> Metadata {
    let udt_info = (name == "UDT").then(|| tedious_rs::metadata_parser::UdtInfo {
        max_byte_size: 3,
        dbname: "db".to_owned(),
        owning_schema: "dbo".to_owned(),
        type_name: "point".to_owned(),
        assembly_name: String::new(),
    });
    Metadata {
        user_type: 0,
        flags: 0,
        data_type: type_by_name(name).unwrap(),
        collation: Some(Collation::new(0x0409, 0, 0, 0)),
        precision: Some(38),
        scale: Some(2),
        data_length: None,
        schema: None,
        udt_info,
    }
}

#[test]
fn nullable_wire_aliases_produce_their_registered_wire_ids() {
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
            .encode(&Value::Null, &options())
            .unwrap();
        assert_eq!(encoded.type_info[0], id, "nullable alias wire id differs");
        assert_eq!(encoded.length, [0]);
        assert!(encoded.data.is_empty());
    }
}

#[test]
fn xml_uses_utf16_plp_and_preserves_null() {
    let datatype = type_by_name("Xml").unwrap();
    let value = datatype
        .encode(&Value::Text("<x>é</x>".to_owned()), &options())
        .unwrap();
    assert_eq!(value.type_info, [0xF1, 0]);
    assert_eq!(
        value.length,
        [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    let mut wire = value.length.clone();
    wire.extend(value.data);
    let parsed = read_value(&wire, 0, &metadata("Xml")).unwrap();
    assert_eq!(parsed.value, Value::Text("<x>é</x>".to_owned()));
    let null = datatype.encode(&Value::Null, &options()).unwrap();
    assert_eq!(null.length, [0xFF; 8]);
    assert!(null.data.is_empty());
    let odd = [0xFE, 1, 0, 0, 0, 0, 0, 0, 0, 0x41];
    assert!(read_value(&odd, 0, &metadata("Xml")).is_err());
}

#[test]
fn sql_variant_round_trips_integer_text_and_decimal_without_f64() {
    let datatype = type_by_name("Variant").unwrap();
    for value in [
        Value::Integer(2_147_483_647),
        Value::Text("variant".to_owned()),
        Value::Decimal {
            unscaled: 12345678901234567890123456789012345678,
            scale: 2,
        },
    ] {
        let encoded = datatype.encode(&value, &options()).unwrap();
        let mut wire = encoded.length.clone();
        wire.extend(encoded.data);
        let parsed = read_value(&wire, 0, &metadata("Variant")).unwrap();
        assert_eq!(parsed.value, value);
        assert_eq!(parsed.offset, wire.len());
    }
}

#[test]
fn udt_has_names_in_type_info_and_plp_payload() {
    let value = Value::Udt {
        database: "db".to_owned(),
        schema: "dbo".to_owned(),
        type_name: "point".to_owned(),
        data: vec![1, 2, 3],
    };
    let encoded = type_by_name("UDT")
        .unwrap()
        .encode(&value, &options())
        .unwrap();
    assert_eq!(encoded.type_info[0], 0xF0);
    assert!(
        encoded
            .type_info
            .windows(4)
            .any(|window| window == [b'd', 0, b'b', 0])
    );
    assert_eq!(
        encoded.length,
        [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    let mut wire = encoded.length.clone();
    wire.extend(encoded.data);
    let parsed = read_value(&wire, 0, &metadata("UDT")).unwrap();
    assert_eq!(parsed.value, Value::Binary(vec![1, 2, 3]));
}

#[test]
fn null_type_has_a_defined_empty_serialization() {
    let encoded = type_by_name("Null")
        .unwrap()
        .encode(&Value::Null, &options())
        .unwrap();
    assert_eq!(encoded.type_info, [0x1F]);
    assert!(encoded.length.is_empty());
    assert!(encoded.data.is_empty());
}
