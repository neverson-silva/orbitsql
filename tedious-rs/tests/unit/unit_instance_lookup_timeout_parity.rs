use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tedious_rs::connector::CancellationToken;
use tedious_rs::instance_lookup::lookup_instance_with_options;
use tokio::net::UdpSocket;

#[tokio::test]
async fn propagates_non_timeout_errors_without_retrying() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed_calls = Arc::clone(&calls);
    let lookup = move |_host: String, _port: u16| {
        observed_calls.fetch_add(1, Ordering::Relaxed);
        async { Err(io::Error::other("non-timeout lookup failure")) }
    };

    let error = lookup_instance_with_options(
        "sql.example",
        "MSSQL",
        Duration::from_millis(10),
        3,
        1434,
        &lookup,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();

    assert!(error.0.contains("non-timeout lookup failure"));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn retries_timeout_errors_until_the_retry_limit() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed_calls = Arc::clone(&calls);
    let lookup = move |_host: String, _port: u16| {
        observed_calls.fetch_add(1, Ordering::Relaxed);
        let address = address;
        async move { Ok::<Vec<SocketAddr>, io::Error>(vec![address]) }
    };

    let error = lookup_instance_with_options(
        "sql.example",
        "MSSQL",
        Duration::from_millis(5),
        2,
        address.port(),
        &lookup,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();

    assert_eq!(
        error.0,
        "Failed to get response from SQL Server Browser on sql.example"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn retains_the_first_response_until_all_retry_attempts_finish() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed_calls = Arc::clone(&calls);
    let responder = tokio::spawn(async move {
        let mut buffer = [0; 128];
        for attempt in 0..3 {
            let (_, peer) = server.recv_from(&mut buffer).await.unwrap();
            observed_calls.fetch_add(1, Ordering::Relaxed);
            if attempt == 0 {
                server
                    .send_to(b"\0\0\0InstanceName;MSSQL;tcp;1433;;", peer)
                    .await
                    .unwrap();
            }
        }
    });

    let port = lookup_instance_with_options(
        "sql.example",
        "MSSQL",
        Duration::from_millis(20),
        2,
        address.port(),
        &move |_host: String, _port: u16| {
            let address = address;
            async move { Ok::<Vec<SocketAddr>, io::Error>(vec![address]) }
        },
        &CancellationToken::new(),
    )
    .await
    .unwrap();

    responder.await.unwrap();
    assert_eq!(port, 1433);
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}
