use tedious_rs::tds_versions::{
    is_tds_8_0, strict_tls_version_is_valid, version_name, version_value,
};
use tedious_rs::transport::{TransportError, connect_strict_tls, connect_tls};
use tokio::net::TcpListener;

#[tokio::test]
async fn rejects_plain_tcp_as_tls_without_disabling_validation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let _ = listener.accept().await.unwrap();
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    assert!(connect_tls(stream, "localhost", false).await.is_err());
    server.await.unwrap();
}

#[test]
fn recognizes_only_the_wire_tds_8_0_version_for_strict_tls() {
    assert!(is_tds_8_0("8_0"));
    assert!(strict_tls_version_is_valid("8_0"));
    assert_eq!(version_value("8_0"), Some(0x08000000));
    assert_eq!(version_name(0x08000000), Some("8_0"));
    assert!(!strict_tls_version_is_valid("7_4"));
    assert!(!strict_tls_version_is_valid("8.0"));
    assert_eq!(version_value("8.0"), None);
}

#[tokio::test]
async fn rejects_invalid_strict_tls_server_names_before_network_io() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let _ = listener.accept().await;
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let error = connect_strict_tls(stream, "not a hostname", true)
        .await
        .unwrap_err();
    assert!(
        matches!(error, TransportError::InvalidPacket(message) if message.contains("invalid TLS server name"))
    );
    server.await.unwrap();
}
