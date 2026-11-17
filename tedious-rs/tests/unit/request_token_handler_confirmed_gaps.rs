use tedious_rs::request::RequestEvent;
use tedious_rs::token::handler::{RequestTokenHandler, TokenHandler};
use tedious_rs::token::rows::ColumnMetadata;

fn column(name: &str) -> ColumnMetadata {
    let mut data = vec![1, 0, 0, 0, 0, 0, 0, 0, 0x38, name.len() as u8];
    data.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
    tedious_rs::token::rows::parse_colmetadata(&data, "7_4", false)
        .expect("fixture construction failed")
        .value[0]
        .clone()
}

#[test]
fn use_column_names_deduplicates_without_sorting_column_metadata() {
    let first = column("z");
    let second = column("a");
    let duplicate = first.clone();
    let mut handler = RequestTokenHandler::new(false, true);

    handler
        .on_col_metadata(&[first.clone(), second.clone(), duplicate])
        .unwrap();

    assert_eq!(handler.columns, vec![first, second]);
    assert!(matches!(
        handler.events.first(),
        Some(RequestEvent::NamedColumnMetadata(columns))
            if columns.len() == 2 && columns.contains_key("z") && columns.contains_key("a")
    ));
    let names: Vec<&str> = match handler.events.first() {
        Some(RequestEvent::NamedColumnMetadata(columns)) => {
            columns.iter().map(|(name, _)| name.as_str()).collect()
        }
        _ => Vec::new(),
    };
    assert_eq!(names, vec!["z", "a"]);
}
