use std::sync::{Arc, Mutex};
use tedious_rs::connection::dispatch_request_tokens;
use tedious_rs::request::{Request, RequestEvent};
use tedious_rs::token::parser::{DoneToken, Token};
use tedious_rs::token::rows::{parse_colmetadata, parse_row};
use tedious_rs::token::stream::StreamToken;
use tedious_rs::value_codec::Value;

fn bvar(value: &str) -> Vec<u8> {
    let units: Vec<u16> = value.encode_utf16().collect();
    [
        vec![units.len() as u8],
        units.into_iter().flat_map(u16::to_le_bytes).collect(),
    ]
    .concat()
}

fn metadata(name: &str) -> tedious_rs::token::rows::ColumnMetadata {
    let mut data = vec![1, 0, 0, 0, 0, 0, 0, 0, 0x38];
    data.extend(bvar(name));
    parse_colmetadata(&data, "7_4", false)
        .unwrap()
        .value
        .remove(0)
}

fn row(column: tedious_rs::token::rows::ColumnMetadata, value: u8) -> tedious_rs::token::rows::Row {
    parse_row(&[value, 0, 0, 0], 0, &[column]).unwrap().value
}

fn done(token: u8) -> StreamToken {
    StreamToken::Control(Token::Done(DoneToken {
        token,
        more: token != 0xFD,
        sql_error: false,
        attention: false,
        server_error: false,
        row_count: Some(1),
        cur_cmd: 0,
    }))
}

#[test]
fn done_events_contain_only_the_current_resultset_rows() {
    let mut request = Request::new("select", |_| {});
    request.begin().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    request.on(move |event| observed.lock().unwrap().push(event.clone()));
    let first_metadata = metadata("first");
    let second_metadata = metadata("second");
    dispatch_request_tokens(
        &mut request,
        vec![
            StreamToken::ColMetadata(vec![first_metadata.clone()]),
            StreamToken::Row(row(first_metadata, 1)),
            done(0xFD),
            StreamToken::ColMetadata(vec![second_metadata.clone()]),
            StreamToken::Row(row(second_metadata, 2)),
            done(0xFD),
        ],
        true,
        false,
        false,
    );
    let observed = events.lock().unwrap();
    let done_rows: Vec<Vec<Value>> = observed
        .iter()
        .filter_map(|event| match event {
            RequestEvent::Done {
                rows: Some(rows), ..
            } => Some(
                rows.iter()
                    .map(|row| row.columns[0].value.clone())
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        done_rows,
        vec![vec![Value::Integer(1)], vec![Value::Integer(2)]]
    );
}

#[test]
fn canceled_request_drops_dispatcher_state_and_response_events() {
    let mut request = Request::new("select", |_| {});
    request.begin().unwrap();
    request.row_count = 7;
    request.return_status = Some(9);
    request.cancel();
    let column = metadata("value");

    dispatch_request_tokens(
        &mut request,
        vec![
            StreamToken::ColMetadata(vec![column.clone()]),
            StreamToken::Row(row(column, 1)),
            done(0xFD),
            StreamToken::Control(Token::ReturnStatus(
                tedious_rs::token::parser::ReturnStatusToken { value: 3 },
            )),
        ],
        true,
        false,
        false,
    );

    assert_eq!(request.rows.len(), 0);
    assert_eq!(request.row_count, 7);
    assert_eq!(request.return_status, Some(9));
}

#[test]
fn use_column_names_records_named_metadata_and_all_duplicate_values() {
    let mut request = Request::new("select", |_| {});
    request.begin().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    request.on(move |event| observed.lock().unwrap().push(event.clone()));
    let first = metadata("value");
    let duplicate = metadata("value");
    dispatch_request_tokens(
        &mut request,
        vec![
            StreamToken::ColMetadata(vec![first.clone(), duplicate]),
            StreamToken::Row(row(first, 7)),
        ],
        false,
        false,
        true,
    );
    let observed = events.lock().unwrap();
    assert!(matches!(
        &observed[0],
            RequestEvent::NamedColumnMetadata(columns)
                if columns.len() == 1 && columns.contains_key("value")
    ));
    assert!(matches!(
        &observed[1],
        RequestEvent::NamedRow(values)
            if values.get("value") == Some(&vec![Value::Integer(7)])
    ));
}

#[test]
fn use_column_names_records_column_metadata_shape() {
    let mut request = Request::new("select", |_| {});
    request.begin().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    request.on(move |event| observed.lock().unwrap().push(event.clone()));
    dispatch_request_tokens(
        &mut request,
        vec![StreamToken::ColMetadata(vec![metadata("value")])],
        false,
        false,
        true,
    );
    let observed = events.lock().unwrap();
    assert!(matches!(
        &observed[0],
            RequestEvent::NamedColumnMetadata(columns)
                if columns.len() == 1 && columns.contains_key("value")
    ));
}

#[test]
fn request_rows_are_collected_only_for_request_completion() {
    let column = metadata("value");
    let value = row(column.clone(), 7);
    let mut request = Request::new("select", |_| {});
    request.begin().unwrap();
    dispatch_request_tokens(
        &mut request,
        vec![StreamToken::Row(value.clone())],
        false,
        false,
        false,
    );
    assert!(request.rows.is_empty());

    let mut request = Request::new("select", |_| {});
    request.begin().unwrap();
    request.set_row_collection_on_completion(true);
    dispatch_request_tokens(
        &mut request,
        vec![StreamToken::Row(value.clone())],
        false,
        false,
        false,
    );
    assert_eq!(request.rows, vec![value]);
}
