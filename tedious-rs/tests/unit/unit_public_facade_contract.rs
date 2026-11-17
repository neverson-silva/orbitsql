use std::sync::{Arc, Mutex};

use tedious_rs::{
    BULK_LOAD_EVENT_NAMES, BulkLoadEvent, BulkLoadEventEmitter, BulkLoadEventName, BulkLoadOptions,
    Connection, ConnectionConfig, ConnectionEvent, ConnectionEventEmitter, ConnectionEventName,
    EVENT_NEW_LISTENER, EVENT_REMOVE_LISTENER, LIBRARY_NAME, NodeBulkLoad, NodeConnection,
    NodeConnectionConfiguration, NodeRequest, NodeRequestError, REQUEST_EVENT_NAMES, Request,
    RequestEvent, RequestEventEmitter, RequestEventName, RequestOptions, RequestResult,
};

#[test]
fn exposes_node_root_aliases_constants_and_event_names() {
    let _: NodeConnection = Connection::new(ConnectionConfig::sql("localhost", "u", "p"));
    let _: NodeConnectionConfiguration = ConnectionConfig::sql("localhost", "u", "p");
    let _: tedious_rs::ConnectionOptions = Default::default();
    let _: NodeRequest = Request::new("select 1", |_| {});
    let _: NodeRequestError = tedious_rs::RequestError::new("x", None);
    assert_eq!(LIBRARY_NAME, "Tedious");
    assert_eq!(EVENT_NEW_LISTENER, "newListener");
    assert_eq!(EVENT_REMOVE_LISTENER, "removeListener");
    assert!(REQUEST_EVENT_NAMES.contains(&"requestCompleted"));
    assert!(BULK_LOAD_EVENT_NAMES.contains(&"error"));
    assert_eq!(ConnectionEventName::Connect.as_str(), "connect");
    assert_eq!(
        RequestEventName::RequestCompleted.as_str(),
        "requestCompleted"
    );
    assert_eq!(BulkLoadEventName::Error.as_str(), "error");
}

#[test]
fn request_facade_preserves_listener_order_once_and_removal() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut request = Request::new("select 1", |_| {});
    let persistent = Arc::clone(&observed);
    let persistent_id = request.on_event(move |event| {
        persistent
            .lock()
            .unwrap()
            .push(matches!(event, RequestEvent::Prepared));
    });
    let once = Arc::clone(&observed);
    request.once_event(move |_| once.lock().unwrap().push(true));
    assert!(request.emit_event_checked(RequestEvent::Prepared));
    assert!(request.emit_event_checked(RequestEvent::Prepared));
    assert_eq!(observed.lock().unwrap().len(), 3);
    assert!(request.off_event(persistent_id));
    assert_eq!(request.remove_all_event_listeners(), 0);
}

#[test]
fn bulk_facade_exposes_checked_emission_and_remove_all() {
    let mut bulk = NodeBulkLoad::new("dbo.items", BulkLoadOptions::default()).unwrap();
    assert!(!bulk.emit_event_checked(BulkLoadEvent::Cancel));
    let id = bulk.on_event(|_| {});
    assert!(bulk.emit_event_checked(BulkLoadEvent::Cancel));
    assert!(bulk.off_event(id));
    assert_eq!(bulk.remove_all_event_listeners(), 0);
}

#[test]
fn connection_facade_exposes_listener_aliases_without_changing_connection_api() {
    let mut connection = Connection::new(ConnectionConfig::sql("localhost", "u", "p"));
    let id = connection.on_event(|event| assert!(matches!(event, ConnectionEvent::Debug(_))));
    assert!(connection.emit_event_checked(ConnectionEvent::Debug("x".to_owned())));
    assert!(connection.off_event(id));
    assert_eq!(connection.remove_all_event_listeners(), 0);
}

#[test]
fn request_type_aliases_remain_usable_as_external_consumer() {
    let options = RequestOptions::default();
    let request = Request::with_options(
        "select 1",
        |result: Result<RequestResult, _>| {
            assert!(result.is_ok());
        },
        options,
    );
    assert_eq!(request.sql_text, "select 1");
}
