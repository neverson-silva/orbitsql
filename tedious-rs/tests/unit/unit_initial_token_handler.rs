use tedious_rs::token::handler::{InitialSqlTokenHandler, TokenHandler, dispatch_stream_token};
use tedious_rs::token::rows::Row;
use tedious_rs::token::stream::StreamToken;

fn assert_rejected(
    result: Result<(), tedious_rs::token::handler::UnexpectedTokenError>,
    handler: &InitialSqlTokenHandler,
    token: &'static str,
    message: &str,
) {
    let error = result.expect_err("initial SQL handler must reject result tokens");
    assert_eq!(error.handler, "InitialSqlTokenHandler");
    assert_eq!(error.token, token);
    assert_eq!(error.category, "error");
    assert_eq!(error.message.as_deref(), Some(message));
    assert!(error.close_connection);
    assert!(handler.close_requested);
    assert_eq!(error.to_string(), message);
}

#[test]
fn colmetadata_requests_node_compatible_error_and_close() {
    let mut handler = InitialSqlTokenHandler::new();
    let result = handler.on_col_metadata(&[]);
    assert_rejected(
        result,
        &handler,
        "COLMETADATA",
        "Received 'columnMetadata' when no sqlRequest is in progress",
    );
}

#[test]
fn order_requests_node_compatible_error_and_close() {
    let mut handler = InitialSqlTokenHandler::new();
    let result = handler.on_order(&[]);
    assert_rejected(
        result,
        &handler,
        "ORDER",
        "Received 'order' when no sqlRequest is in progress",
    );
}

#[test]
fn row_requests_node_compatible_error_and_close() {
    let mut handler = InitialSqlTokenHandler::new();
    let result = handler.on_row(&Row {
        columns: Vec::new(),
    });
    assert_rejected(
        result,
        &handler,
        "ROW",
        "Received 'row' when no sqlRequest is in progress",
    );
}

#[test]
fn nbcrow_requests_node_compatible_error_and_close() {
    let mut handler = InitialSqlTokenHandler::new();
    let result = handler.on_nbc_row(&Row {
        columns: Vec::new(),
    });
    assert_rejected(
        result,
        &handler,
        "NBCROW",
        "Received 'row' when no sqlRequest is in progress",
    );
}

#[test]
fn stream_dispatch_preserves_initial_handler_rejection_for_each_result_token() {
    let cases = [
        (
            StreamToken::ColMetadata(Vec::new()),
            "COLMETADATA",
            "Received 'columnMetadata' when no sqlRequest is in progress",
        ),
        (
            StreamToken::Row(Row {
                columns: Vec::new(),
            }),
            "ROW",
            "Received 'row' when no sqlRequest is in progress",
        ),
        (
            StreamToken::NbcRow(Row {
                columns: Vec::new(),
            }),
            "NBCROW",
            "Received 'row' when no sqlRequest is in progress",
        ),
    ];

    for (token, name, message) in cases {
        let mut handler = InitialSqlTokenHandler::new();
        assert_rejected(
            dispatch_stream_token(&mut handler, &token),
            &handler,
            name,
            message,
        );
    }
}
