use tedious_rs::token::rows::{parse_colmetadata, parse_nbc_row, parse_row};
use tedious_rs::value_codec::Value;

fn bvar(value: &str) -> Vec<u8> {
    let units: Vec<u16> = value.encode_utf16().collect();
    [
        vec![units.len() as u8],
        units.into_iter().flat_map(u16::to_le_bytes).collect(),
    ]
    .concat()
}

fn columns() -> Vec<u8> {
    let mut data = vec![2, 0, 0, 0, 0, 0, 0, 0, 0x38];
    data.extend(bvar("Id"));
    data.extend([0, 0, 0, 0, 0, 0, 0xA7, 5, 0, 9, 4, 0, 0, 52]);
    data.extend(bvar("Name"));
    data
}

#[test]
fn parses_column_metadata_and_rows_in_order() {
    let metadata = parse_colmetadata(&columns(), "7_4", false).unwrap();
    assert_eq!(metadata.value.len(), 2);
    assert_eq!(metadata.value[1].col_name, "Name");
    let row = parse_row(&[7, 0, 0, 0, 2, 0, 0x41, 0x42], 0, &metadata.value).unwrap();
    assert_eq!(row.value.columns[0].value, Value::Integer(7));
    assert_eq!(row.value.columns[1].value, Value::Text("AB".to_owned()));
    assert_eq!(row.offset, 8);
    let named = row.value.named_values();
    assert_eq!(named.get("Id").unwrap(), &vec![Value::Integer(7)]);
    assert_eq!(
        named.get("Name").unwrap(),
        &vec![Value::Text("AB".to_owned())]
    );
}

#[test]
fn parses_nbc_bitmap_and_skips_null_value_bytes() {
    let metadata = parse_colmetadata(&columns(), "7_4", true).unwrap();
    let row = parse_nbc_row(&[1, 1, 0, 0], 0, &metadata.value).unwrap();
    assert_eq!(row.value.columns[0].value, Value::Null);
    assert_eq!(
        row.value.columns[1].value,
        Value::Text("\u{0000}".to_owned())
    );
}

#[test]
fn rejects_truncated_column_and_row_payloads() {
    assert!(parse_colmetadata(&[1, 0, 0], "7_4", false).is_err());
    let metadata = parse_colmetadata(&columns(), "7_4", false).unwrap();
    assert!(parse_row(&[1], 0, &metadata.value).is_err());
    assert!(parse_nbc_row(&[0], 0, &metadata.value).is_err());
}

#[test]
fn parses_legacy_text_table_name_for_tds_70() {
    let mut columns = vec![1, 0, 0x34, 0x12, 0, 0, 0x23];
    columns.extend(u32::MAX.to_le_bytes());
    columns.extend([9, 4, 0, 0, 52]);
    columns.extend([3, 0, b'd', 0, b'b', 0, b'o', 0]);
    columns.extend(bvar("Body"));

    let parsed = parse_colmetadata(&columns, "7_0", false).unwrap();
    assert_eq!(parsed.offset, columns.len());
    assert_eq!(parsed.value[0].col_name, "Body");
    assert_eq!(
        parsed.value[0].table_name,
        Some(tedious_rs::token::rows::TableName::Legacy("dbo".to_owned()))
    );
}

#[test]
fn named_values_keeps_all_values_for_duplicate_column_names() {
    let metadata = parse_colmetadata(&columns(), "7_4", false).unwrap().value;
    let row = tedious_rs::token::rows::Row {
        columns: vec![
            tedious_rs::token::rows::RowColumn {
                value: Value::Integer(7),
                metadata: metadata[0].clone(),
            },
            tedious_rs::token::rows::RowColumn {
                value: Value::Integer(8),
                metadata: metadata[0].clone(),
            },
        ],
    };
    let named = row.named_values();
    assert_eq!(
        named.get("Id"),
        Some(&vec![Value::Integer(7), Value::Integer(8)])
    );
}
