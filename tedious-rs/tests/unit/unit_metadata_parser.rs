use tedious_rs::data_type::{type_by_id, type_by_name};
use tedious_rs::metadata_parser::{parse_metadata, read_metadata};

fn bvar(value: &str) -> Vec<u8> {
    let units: Vec<u16> = value.encode_utf16().collect();
    let mut result = vec![units.len() as u8];
    for unit in units {
        result.extend(unit.to_le_bytes());
    }
    result
}

fn usvar(value: &str) -> Vec<u8> {
    let units: Vec<u16> = value.encode_utf16().collect();
    let mut result = (units.len() as u16).to_le_bytes().to_vec();
    for unit in units {
        result.extend(unit.to_le_bytes());
    }
    result
}

fn base_74(id: u8) -> Vec<u8> {
    [0x44, 0x33, 0x22, 0x11, 0x07, 0x00, id].to_vec()
}

#[test]
fn registry_matches_wire_ids_and_names() {
    assert_eq!(type_by_id(0xA7).unwrap().name, "VarChar");
    assert_eq!(type_by_id(0xF3).unwrap().name, "TVP");
    assert_eq!(type_by_name("UniqueIdentifier").unwrap().id, 0x24);
    assert!(type_by_id(0xFF).is_none());
}

#[test]
fn reads_fixed_and_tds_71_user_type_width() {
    let fixed = base_74(0x38);
    let parsed = read_metadata(&fixed, 0, "7_4").unwrap();
    assert_eq!(parsed.value.user_type, 0x11223344);
    assert_eq!(parsed.value.data_type.name, "Int");
    assert_eq!(parsed.offset, 7);

    let old = [0x34, 0x12, 0x07, 0x00, 0x38].to_vec();
    let parsed = read_metadata(&old, 0, "7_1").unwrap();
    assert_eq!(parsed.value.user_type, 0x1234);
    assert_eq!(parsed.offset, 5);

    let parsed = read_metadata(&old, 0, "7_0").unwrap();
    assert_eq!(parsed.value.user_type, 0x1234);
    assert_eq!(parsed.offset, 5);
}

#[test]
fn reads_variable_binary_and_text_metadata() {
    let mut binary = base_74(0xA5);
    binary.extend([0x20, 0x00]);
    let parsed = read_metadata(&binary, 0, "7_4").unwrap();
    assert_eq!(parsed.value.data_length, Some(32));

    let mut varchar = base_74(0xA7);
    varchar.extend([0x10, 0x00, 0x09, 0x04, 0x00, 0x00, 52]);
    let parsed = read_metadata(&varchar, 0, "7_4").unwrap();
    assert_eq!(parsed.value.data_length, Some(16));
    assert_eq!(
        parsed
            .value
            .collation
            .unwrap()
            .codepage
            .unwrap()
            .to_string(),
        "CP1252"
    );
}

#[test]
fn reads_numeric_temporal_xml_and_udt_metadata() {
    let mut numeric = base_74(0x6A);
    numeric.extend([17, 12, 3]);
    let parsed = read_metadata(&numeric, 0, "7_4").unwrap();
    assert_eq!(parsed.value.data_length, Some(17));
    assert_eq!(parsed.value.precision, Some(12));
    assert_eq!(parsed.value.scale, Some(3));

    let mut temporal = base_74(0x2B);
    temporal.push(7);
    assert_eq!(
        read_metadata(&temporal, 0, "7_4").unwrap().value.scale,
        Some(7)
    );

    let mut xml = base_74(0xF1);
    xml.push(1);
    xml.extend(bvar("db"));
    xml.extend(bvar("dbo"));
    xml.extend(usvar("docs"));
    let parsed = read_metadata(&xml, 0, "7_4").unwrap();
    assert_eq!(parsed.value.schema.unwrap().dbname, "db");

    let mut udt = base_74(0xF0);
    udt.extend([0x20, 0x00]);
    udt.extend(bvar("db"));
    udt.extend(bvar("dbo"));
    udt.extend(bvar("Point"));
    udt.extend(usvar("Assembly"));
    let parsed = read_metadata(&udt, 0, "7_4").unwrap();
    assert_eq!(parsed.value.udt_info.unwrap().type_name, "Point");
}

#[test]
fn reads_smallmoney_metadata_without_treating_it_as_unknown() {
    let parsed = read_metadata(&base_74(0x7A), 0, "7_4").unwrap();
    assert_eq!(parsed.value.data_type.name, "SmallMoney");
    assert_eq!(parsed.offset, 7);
}

#[test]
fn reads_legacy_decimal_and_numeric_metadata() {
    for id in [0x37, 0x3F] {
        let mut data = base_74(id);
        data.extend([5, 12, 3]);
        let parsed = read_metadata(&data, 0, "7_4").unwrap();
        assert_eq!(parsed.value.data_length, Some(5));
        assert_eq!(parsed.value.precision, Some(12));
        assert_eq!(parsed.value.scale, Some(3));
        assert_eq!(parsed.offset, data.len());
    }
}

#[test]
fn parses_concatenated_metadata_and_reports_wire_errors() {
    let mut data = base_74(0x38);
    data.extend(base_74(0x32));
    let values = parse_metadata(&data, "7_4").unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[1].data_type.name, "Bit");

    let invalid = [0, 0, 0, 0, 0, 0, 0xFF];
    let error = read_metadata(&invalid, 0, "7_4").unwrap_err();
    assert!(format!("{error:?}").contains("Unrecognised data type"));
}
