use std::sync::{Arc, Mutex};

use tedious_rs::incoming_message_stream::{IncomingMessageStream, MessageStreamEvent};
use tedious_rs::message::{Message, packetize};
use tedious_rs::packet::packet_type;
use tedious_rs::request::{Request, RequestEvent, RequestResult};
use tedious_rs::token::rows::Row;

#[test]
fn request_pause_resume_is_idempotent_and_orders_lifecycle_events() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", |_result: Result<RequestResult, _>| {});
    request.on(move |event| observed.lock().unwrap().push(event.clone()));

    request.pause();
    request.pause();
    request.resume();
    request.resume();

    assert!(!request.paused);
    assert_eq!(
        *events.lock().unwrap(),
        vec![RequestEvent::Pause, RequestEvent::Resume]
    );
}

#[test]
fn request_resume_releases_events_in_arrival_order_after_resume_event() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", |_result: Result<RequestResult, _>| {});
    request.on(move |event| observed.lock().unwrap().push(event.clone()));

    request.pause();
    request.emit(RequestEvent::ColumnMetadata(Vec::new()));
    request.emit(RequestEvent::Row(Row {
        columns: Vec::new(),
    }));
    request.emit(RequestEvent::Done {
        row_count: Some(1),
        more: false,
        rows: None,
    });

    assert_eq!(events.lock().unwrap().as_slice(), &[RequestEvent::Pause]);

    request.resume();
    let events = events.lock().unwrap();
    assert_eq!(events[0], RequestEvent::Pause);
    assert_eq!(events[1], RequestEvent::Resume);
    assert!(matches!(events[2], RequestEvent::ColumnMetadata(_)));
    assert!(matches!(events[3], RequestEvent::Row(_)));
    assert!(matches!(events[4], RequestEvent::Done { .. }));
}

#[test]
fn incoming_stream_pause_retains_complete_wire_data_until_resume() {
    let first = message(packet_type::TABULAR_RESULT, b"first");
    let second = message(packet_type::TABULAR_RESULT, b"second");
    let mut wire = packetize(&first, 64).unwrap()[0].buffer.clone();
    wire.extend_from_slice(&packetize(&second, 64).unwrap()[0].buffer);

    let mut stream = IncomingMessageStream::new();
    stream.pause();
    assert!(stream.feed(&wire).unwrap().is_empty());
    assert!(stream.is_paused());
    assert_eq!(stream.buffered_len(), wire.len());

    let messages = stream.resume().unwrap();
    assert!(!stream.is_paused());
    assert_eq!(messages, vec![first, second]);
    assert_eq!(stream.buffered_len(), 0);
}

#[test]
fn incoming_stream_pause_resume_preserves_partial_packet_boundaries() {
    let message = message(packet_type::RPC_REQUEST, b"payload");
    let wire = packetize(&message, 64).unwrap()[0].buffer.clone();
    let split = 3;
    let mut stream = IncomingMessageStream::new();

    assert!(stream.feed(&wire[..split]).unwrap().is_empty());
    stream.pause();
    assert!(stream.feed(&wire[split..]).unwrap().is_empty());
    assert_eq!(stream.buffered_len(), wire.len());

    assert_eq!(stream.resume().unwrap(), vec![message]);
    assert_eq!(stream.buffered_len(), 0);
}

#[test]
fn incoming_stream_incremental_resume_emits_one_message_lifecycle() {
    let message = message(packet_type::PRELOGIN, b"payload");
    let wire = packetize(&message, 64).unwrap()[0].buffer.clone();
    let mut stream = IncomingMessageStream::new();

    stream.pause();
    assert!(stream.feed_incremental(&wire).unwrap().is_empty());
    assert_eq!(
        stream.resume_incremental().unwrap(),
        vec![
            MessageStreamEvent::MessageStarted(Message::new(packet_type::PRELOGIN)),
            MessageStreamEvent::Data {
                message_type: packet_type::PRELOGIN,
                data: b"payload".to_vec(),
            },
            MessageStreamEvent::MessageEnded {
                message_type: packet_type::PRELOGIN,
            },
        ]
    );
    assert!(stream.resume_incremental().unwrap().is_empty());
}

fn message(message_type: u8, data: &[u8]) -> Message {
    Message {
        message_type,
        reset_connection: false,
        ignore: false,
        data: data.to_vec(),
    }
}
