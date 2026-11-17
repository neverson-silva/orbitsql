use std::time::Duration;
use tedious_rs::sender::send_in_parallel;
use tokio::net::UdpSocket;

#[tokio::test]
async fn sends_udp_request_and_returns_first_response() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().unwrap().port();
    let server_task = tokio::spawn(async move {
        let mut buffer = [0; 32];
        let (length, peer) = server.recv_from(&mut buffer).await.unwrap();
        assert_eq!(&buffer[..length], b"request");
        server.send_to(b"response", peer).await.unwrap();
    });
    let response = send_in_parallel(
        &["127.0.0.1:0".parse().unwrap()],
        port,
        b"request",
        Duration::from_secs(1),
    )
    .await
    .unwrap();
    assert_eq!(response, b"response");
    server_task.await.unwrap();
}

#[tokio::test]
async fn reports_udp_timeout_when_no_server_responds() {
    let result = send_in_parallel(
        &["127.0.0.1:0".parse().unwrap()],
        9,
        b"request",
        Duration::from_millis(20),
    )
    .await;
    assert_eq!(result.unwrap_err(), "UDP response timeout");
}
