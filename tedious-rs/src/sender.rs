use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::connector::{
    AddressLookup, CancellationToken, SystemAddressLookup, lookup_all_addresses_with,
};

pub async fn send_in_parallel(
    addresses: &[SocketAddr],
    port: u16,
    request: &[u8],
    operation_timeout: Duration,
) -> Result<Vec<u8>, String> {
    send_in_parallel_with_cancel(
        addresses,
        port,
        request,
        operation_timeout,
        &CancellationToken::new(),
    )
    .await
}

pub async fn send_in_parallel_with_cancel(
    addresses: &[SocketAddr],
    port: u16,
    request: &[u8],
    operation_timeout: Duration,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, String> {
    if addresses.is_empty() {
        return Err("host resolved to no addresses".to_owned());
    }
    if cancellation.is_cancelled() {
        return Err("operation cancelled".to_owned());
    }
    let mut tasks = JoinSet::new();
    for address in addresses {
        let address = *address;
        let request = request.to_vec();
        let cancellation = cancellation.clone();
        tasks.spawn(async move {
            let bind_address = if address.is_ipv6() {
                "[::]:0"
            } else {
                "0.0.0.0:0"
            };
            let socket = UdpSocket::bind(bind_address)
                .await
                .map_err(|error| error.to_string())?;
            socket
                .send_to(&request, SocketAddr::new(address.ip(), port))
                .await
                .map_err(|error| error.to_string())?;
            let mut response = vec![0; 65535];
            let received = tokio::select! {
                result = timeout(operation_timeout, socket.recv_from(&mut response)) => result
                    .map_err(|_| "UDP response timeout".to_owned())
                    .and_then(|result| result.map_err(|error| error.to_string())),
                _ = cancellation.cancelled() => Err("operation cancelled".to_owned()),
            }?;
            let (length, _) = received;
            response.truncate(length);
            Ok::<Vec<u8>, String>(response)
        });
    }
    let mut last_error = None;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(response)) => {
                tasks.abort_all();
                return Ok(response);
            }
            Ok(Err(error)) => last_error = Some(error),
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    Err(last_error.unwrap_or_else(|| "UDP send failed".to_owned()))
}

pub async fn send_message(
    host: &str,
    port: u16,
    request: &[u8],
    operation_timeout: Duration,
) -> Result<Vec<u8>, String> {
    send_message_with_lookup(
        host,
        port,
        request,
        operation_timeout,
        &SystemAddressLookup,
        &CancellationToken::new(),
    )
    .await
}

pub async fn send_message_with_lookup(
    host: &str,
    port: u16,
    request: &[u8],
    operation_timeout: Duration,
    lookup: &dyn AddressLookup,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, String> {
    let addresses = lookup_all_addresses_with(host, port, lookup, cancellation)
        .await
        .map_err(|error| format!("{error:?}"))?;
    send_in_parallel_with_cancel(&addresses, port, request, operation_timeout, cancellation).await
}
