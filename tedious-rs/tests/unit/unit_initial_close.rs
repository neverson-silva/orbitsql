use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tedious_rs::token::handler::{InitialSqlTokenHandler, TokenHandler, dispatch_stream_token};
use tedious_rs::token::rows::Row;
use tedious_rs::token::stream::StreamToken;

#[test]
fn unexpected_initial_token_invokes_close_control_once() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut handler = InitialSqlTokenHandler::with_close_callback(move || {
        observed.fetch_add(1, Ordering::SeqCst);
    });

    let first = handler.on_row(&Row {
        columns: Vec::new(),
    });
    let second = handler.on_nbc_row(&Row {
        columns: Vec::new(),
    });

    assert!(first.is_err());
    assert!(second.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(handler.close_requested);
}

#[test]
fn stream_dispatch_invokes_close_control_for_unexpected_result_token() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut handler = InitialSqlTokenHandler::with_close_callback(move || {
        observed.fetch_add(1, Ordering::SeqCst);
    });

    let result = dispatch_stream_token(&mut handler, &StreamToken::ColMetadata(Vec::new()));

    let error = result.expect_err("unexpected initial result token must fail");
    assert_eq!(error.category, "error");
    assert!(error.close_connection);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
