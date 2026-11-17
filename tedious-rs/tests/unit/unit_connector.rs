use std::net::SocketAddr;
use std::time::Duration;

use tedious_rs::connector::{
    AddressLookup, CancellationToken, ConnectorError, LookupFuture,
    connect_in_parallel_with_options_and_lookup, connect_in_sequence_with_options_and_lookup,
    lookup_all_addresses, normalize_host,
};
use tedious_rs::transport::{TransportError, connect_tcp_with_lookup};

#[test]
fn normalizes_idn_hosts_to_ascii_before_resolution() {
    assert_eq!(normalize_host("本地主机.ad"), "xn--tiq21tzznxb.ad");
    assert_eq!(normalize_host("localhost"), "localhost");
    assert_eq!(normalize_host("127.0.0.1"), "127.0.0.1");
}

#[tokio::test]
async fn resolves_numeric_loopback_without_external_credentials() {
    let addresses = lookup_all_addresses("127.0.0.1", 1433).await.unwrap();
    assert!(addresses.iter().any(|address| address.ip().is_loopback()));
}

#[tokio::test]
async fn reports_resolution_failure_for_an_invalid_host() {
    assert!(
        lookup_all_addresses("host-that-does-not-exist.invalid", 1433)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn applies_keep_alive_to_a_connected_socket() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accept = tokio::spawn(async move { listener.accept().await.unwrap() });
    let stream = tedious_rs::connector::connect_in_sequence_with_keep_alive(
        "127.0.0.1",
        port,
        Duration::from_secs(1),
        Some(Duration::from_secs(30)),
    )
    .await
    .unwrap();
    assert!(stream.peer_addr().is_ok());
    accept.await.unwrap();
}

#[tokio::test]
async fn binds_a_configured_local_address_before_connecting() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accept = tokio::spawn(async move { listener.accept().await.unwrap().1 });
    let stream = tedious_rs::connector::connect_in_sequence_with_options(
        "127.0.0.1",
        port,
        Duration::from_secs(1),
        None,
        Some("127.0.0.1"),
    )
    .await
    .unwrap();
    assert_eq!(
        stream.local_addr().unwrap().ip(),
        "127.0.0.1".parse::<std::net::IpAddr>().unwrap()
    );
    drop(stream);
    let _ = accept.await.unwrap();
}

struct DelayedLookup;

impl AddressLookup for DelayedLookup {
    fn lookup(&self, _host: String, _port: u16) -> LookupFuture {
        Box::pin(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(vec!["127.0.0.1:1".parse::<SocketAddr>().unwrap()])
        })
    }
}

#[tokio::test]
async fn sequence_connect_timeout_includes_address_lookup() {
    let result = connect_in_sequence_with_options_and_lookup(
        "delayed.invalid",
        1433,
        Duration::from_millis(10),
        None,
        None,
        &DelayedLookup,
        &CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(result, Err(ConnectorError::AllFailed(errors)) if errors == vec!["connection timeout"])
    );
}

#[tokio::test]
async fn parallel_connect_timeout_includes_address_lookup() {
    let result = connect_in_parallel_with_options_and_lookup(
        "delayed.invalid",
        1433,
        Duration::from_millis(10),
        None,
        None,
        &DelayedLookup,
        &CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(result, Err(ConnectorError::AllFailed(errors)) if errors == vec!["connection timeout"])
    );
}

#[tokio::test]
async fn transport_connect_timeout_includes_address_lookup() {
    let result = connect_tcp_with_lookup(
        "delayed.invalid",
        1433,
        Duration::from_millis(10),
        &DelayedLookup,
        &CancellationToken::new(),
    )
    .await;
    assert!(matches!(result, Err(TransportError::Timeout)));
}
