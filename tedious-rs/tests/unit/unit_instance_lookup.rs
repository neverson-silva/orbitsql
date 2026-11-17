use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use tedious_rs::connector::CancellationToken;
use tedious_rs::instance_lookup::{lookup_instance_with_options, parse_browser_response};
use tokio::net::UdpSocket;

#[test]
fn selects_tcp_port_only_from_the_requested_instance_case_insensitively() {
    let response = "ServerName;host;InstanceName;OTHER;tcp;1444;;ServerName;host;InstanceName;MSSQL;tcp;1433;;";
    assert_eq!(parse_browser_response(response, "mssql"), Some(1433));
    assert_eq!(parse_browser_response(response, "missing"), None);
}

#[test]
fn ignores_invalid_or_unrelated_browser_fields() {
    assert_eq!(
        parse_browser_response("InstanceName;MSSQL;np;pipe;tcp;not-a-port;;", "MSSQL"),
        None
    );
}

#[test]
fn follows_browser_field_case_and_javascript_integer_prefix_rules() {
    assert_eq!(
        parse_browser_response("InstanceName;MSSQL;tcp;1444-extra;;", "MSSQL"),
        Some(1444)
    );
    assert_eq!(
        parse_browser_response("instancename;MSSQL;tcp;1444;;", "MSSQL"),
        None
    );
}

#[test]
fn compares_instance_names_with_javascript_uppercase_semantics() {
    assert_eq!(
        parse_browser_response("InstanceName;straße;tcp;1444;;", "STRASSE"),
        Some(1444)
    );
}

#[tokio::test]
async fn accepts_an_empty_server_name() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let responder = tokio::spawn(async move {
        let mut request = [0; 8];
        let (length, peer) = server.recv_from(&mut request).await.unwrap();
        assert_eq!(&request[..length], &[2]);
        server
            .send_to(b"xyzInstanceName;MSSQL;tcp;1433;;", peer)
            .await
            .unwrap();
    });
    let lookup = move |_host: String, _port: u16| async move {
        Ok::<Vec<SocketAddr>, io::Error>(vec![address])
    };

    let result = lookup_instance_with_options(
        "",
        "MSSQL",
        Duration::from_secs(1),
        0,
        address.port(),
        &lookup,
        &CancellationToken::new(),
    )
    .await;

    assert_eq!(result.unwrap(), 1433);
    responder.await.unwrap();
}

#[tokio::test]
async fn reports_the_node_timeout_message() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let lookup = move |_host: String, _port: u16| async move {
        Ok::<Vec<SocketAddr>, io::Error>(vec![address])
    };

    let error = lookup_instance_with_options(
        "sql.example",
        "MSSQL",
        Duration::from_millis(5),
        0,
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
}

#[tokio::test]
async fn reports_the_node_missing_instance_message_and_rejects_zero_port() {
    let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = server.local_addr().unwrap();
    let responder = tokio::spawn(async move {
        let mut request = [0; 8];
        let (_, peer) = server.recv_from(&mut request).await.unwrap();
        server
            .send_to(b"xyzInstanceName;MSSQL;tcp;0;;", peer)
            .await
            .unwrap();
    });
    let lookup = move |_host: String, _port: u16| async move {
        Ok::<Vec<SocketAddr>, io::Error>(vec![address])
    };

    let error = lookup_instance_with_options(
        "sql.example",
        "MSSQL",
        Duration::from_secs(1),
        0,
        address.port(),
        &lookup,
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();

    assert_eq!(error.0, "Port for MSSQL not found in sql.example");
    responder.await.unwrap();
}
