use tedious_rs::public_api::RequestEventName;
use tedious_rs::request::{NamedColumnMetadata, RequestEvent};
use tedious_rs::token::rows::{ColumnMetadata, Row, RowColumn, parse_colmetadata};
use tedious_rs::value_codec::Value;

fn metadata(name: &str) -> ColumnMetadata {
    let units: Vec<u16> = name.encode_utf16().collect();
    let mut data = vec![1, 0, 0, 0, 0, 0, 0, 0, 0x38, units.len() as u8];
    data.extend(units.into_iter().flat_map(u16::to_le_bytes));
    parse_colmetadata(&data, "7_4", false)
        .unwrap()
        .value
        .remove(0)
}

#[test]
fn named_row_event_type_can_represent_duplicate_column_values() {
    let event = RequestEvent::NamedRow(std::collections::BTreeMap::from([(
        "value".to_owned(),
        vec![Value::Integer(7), Value::Integer(8)],
    )]));

    assert!(matches!(
        event,
        RequestEvent::NamedRow(values)
            if values.get("value") == Some(&vec![Value::Integer(7), Value::Integer(8)])
    ));
}

#[test]
fn named_values_keeps_all_duplicate_columns() {
    let first = metadata("value");
    let second = first.clone();
    let values = Row {
        columns: vec![
            RowColumn {
                value: Value::Integer(7),
                metadata: first,
            },
            RowColumn {
                value: Value::Integer(8),
                metadata: second,
            },
        ],
    }
    .named_values();

    assert_eq!(
        values.get("value"),
        Some(&vec![Value::Integer(7), Value::Integer(8)])
    );
}

#[test]
fn column_metadata_event_type_preserves_node_named_shape() {
    let mut columns = NamedColumnMetadata::new();
    columns.insert("value".to_owned(), metadata("value"));
    let event = RequestEvent::NamedColumnMetadata(columns);

    assert!(
        matches!(event, RequestEvent::NamedColumnMetadata(values) if values.contains_key("value"))
    );
    assert_eq!(RequestEventName::ColumnMetadata.as_str(), "columnMetadata");
}
