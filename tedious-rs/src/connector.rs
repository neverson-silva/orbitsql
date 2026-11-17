use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

pub type LookupFuture = Pin<Box<dyn Future<Output = io::Result<Vec<SocketAddr>>> + Send>>;

pub trait AddressLookup: Send + Sync {
    fn lookup(&self, host: String, port: u16) -> LookupFuture;
}

impl<F, Fut> AddressLookup for F
where
    F: Fn(String, u16) -> Fut + Send + Sync,
    Fut: Future<Output = io::Result<Vec<SocketAddr>>> + Send + 'static,
{
    fn lookup(&self, host: String, port: u16) -> LookupFuture {
        Box::pin(self(host, port))
    }
}

#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        loop {
            self.notify.notified().await;
            if self.is_cancelled() {
                return;
            }
        }
    }
}

pub struct SystemAddressLookup;

impl AddressLookup for SystemAddressLookup {
    fn lookup(&self, host: String, port: u16) -> LookupFuture {
        Box::pin(async move {
            tokio::net::lookup_host((host, port))
                .await
                .map(|items| items.collect())
        })
    }
}

#[derive(Debug)]
pub enum ConnectorError {
    Resolve(io::Error),
    NoAddresses,
    AllFailed(Vec<String>),
    SocketOptions(io::Error),
    Cancelled,
}

pub async fn lookup_all_addresses(
    host: &str,
    port: u16,
) -> Result<Vec<SocketAddr>, ConnectorError> {
    lookup_all_addresses_with(host, port, &SystemAddressLookup, &CancellationToken::new()).await
}

pub async fn lookup_all_addresses_with(
    host: &str,
    port: u16,
    lookup: &dyn AddressLookup,
    cancellation: &CancellationToken,
) -> Result<Vec<SocketAddr>, ConnectorError> {
    if cancellation.is_cancelled() {
        return Err(ConnectorError::Cancelled);
    }
    if let Ok(address) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(address, port)]);
    }
    let host = normalize_host(host);
    let addresses = tokio::select! {
        result = lookup.lookup(host, port) => result.map_err(ConnectorError::Resolve)?,
        _ = cancellation.cancelled() => return Err(ConnectorError::Cancelled),
    };
    if addresses.is_empty() {
        Err(ConnectorError::NoAddresses)
    } else {
        Ok(addresses)
    }
}

pub fn normalize_host(host: &str) -> String {
    match url::Host::parse(host) {
        Ok(url::Host::Domain(value)) => value,
        Ok(url::Host::Ipv4(value)) => value.to_string(),
        Ok(url::Host::Ipv6(value)) => value.to_string(),
        Err(_) => host.to_owned(),
    }
}

pub async fn connect_in_sequence(
    host: &str,
    port: u16,
    connect_timeout: Duration,
) -> Result<TcpStream, ConnectorError> {
    connect_in_sequence_with_keep_alive(host, port, connect_timeout, None).await
}

pub async fn connect_in_sequence_with_keep_alive(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    keep_alive: Option<Duration>,
) -> Result<TcpStream, ConnectorError> {
    connect_in_sequence_with_options(host, port, connect_timeout, keep_alive, None).await
}

pub async fn connect_in_sequence_with_options(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    keep_alive: Option<Duration>,
    local_address: Option<&str>,
) -> Result<TcpStream, ConnectorError> {
    connect_in_sequence_with_options_and_lookup(
        host,
        port,
        connect_timeout,
        keep_alive,
        local_address,
        &SystemAddressLookup,
        &CancellationToken::new(),
    )
    .await
}

pub async fn connect_in_sequence_with_options_and_lookup(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    keep_alive: Option<Duration>,
    local_address: Option<&str>,
    lookup: &dyn AddressLookup,
    cancellation: &CancellationToken,
) -> Result<TcpStream, ConnectorError> {
    let deadline = Instant::now() + connect_timeout;
    let addresses = match timeout(
        connect_timeout,
        lookup_all_addresses_with(host, port, lookup, cancellation),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            return Err(ConnectorError::AllFailed(vec![
                "connection timeout".to_owned(),
            ]));
        }
    };
    let local_address = parse_local_address(local_address)?;
    let mut errors = Vec::new();
    for address in addresses {
        let Some(remaining) = remaining_until(deadline) else {
            errors.push("connection timeout".to_owned());
            break;
        };
        let result = tokio::select! {
            result = timeout(remaining, connect_socket(address, local_address)) => result,
            _ = cancellation.cancelled() => return Err(ConnectorError::Cancelled),
        };
        match result {
            Ok(Ok(stream)) => return apply_keep_alive(stream, keep_alive),
            Ok(Err(error)) => errors.push(error.to_string()),
            Err(_) => errors.push("connection timeout".to_owned()),
        }
    }
    Err(ConnectorError::AllFailed(errors))
}

pub async fn connect_in_parallel(
    host: &str,
    port: u16,
    connect_timeout: Duration,
) -> Result<TcpStream, ConnectorError> {
    connect_in_parallel_with_keep_alive(host, port, connect_timeout, None).await
}

pub async fn connect_in_parallel_with_keep_alive(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    keep_alive: Option<Duration>,
) -> Result<TcpStream, ConnectorError> {
    connect_in_parallel_with_options(host, port, connect_timeout, keep_alive, None).await
}

pub async fn connect_in_parallel_with_options(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    keep_alive: Option<Duration>,
    local_address: Option<&str>,
) -> Result<TcpStream, ConnectorError> {
    connect_in_parallel_with_options_and_lookup(
        host,
        port,
        connect_timeout,
        keep_alive,
        local_address,
        &SystemAddressLookup,
        &CancellationToken::new(),
    )
    .await
}

pub async fn connect_in_parallel_with_options_and_lookup(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    keep_alive: Option<Duration>,
    local_address: Option<&str>,
    lookup: &dyn AddressLookup,
    cancellation: &CancellationToken,
) -> Result<TcpStream, ConnectorError> {
    let deadline = Instant::now() + connect_timeout;
    let addresses = match timeout(
        connect_timeout,
        lookup_all_addresses_with(host, port, lookup, cancellation),
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            return Err(ConnectorError::AllFailed(vec![
                "connection timeout".to_owned(),
            ]));
        }
    };
    let local_address = parse_local_address(local_address)?;
    if cancellation.is_cancelled() {
        return Err(ConnectorError::Cancelled);
    }
    let cancellation = cancellation.clone();
    let address_count = addresses.len();
    let mut tasks = JoinSet::new();
    for (index, address) in addresses.into_iter().enumerate() {
        let cancellation = cancellation.clone();
        let Some(remaining) = remaining_until(deadline) else {
            break;
        };
        tasks.spawn(async move {
            let result = tokio::select! {
                result = timeout(remaining, connect_socket(address, local_address)) => Some(result),
                _ = cancellation.cancelled() => None,
            };
            (index, result)
        });
    }
    let mut errors = Vec::with_capacity(address_count);
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((_, Some(Ok(Ok(stream))))) => {
                tasks.abort_all();
                return apply_keep_alive(stream, keep_alive);
            }
            Ok((_, Some(Ok(Err(error))))) => errors.push(error.to_string()),
            Ok((_, None)) => {
                tasks.abort_all();
                return Err(ConnectorError::Cancelled);
            }
            Ok((_, Some(Err(_)))) => errors.push("connection timeout".to_owned()),
            Err(error) => errors.push(error.to_string()),
        }
    }
    Err(ConnectorError::AllFailed(errors))
}

fn remaining_until(deadline: Instant) -> Option<Duration> {
    deadline.checked_duration_since(Instant::now())
}

fn apply_keep_alive(
    stream: TcpStream,
    keep_alive: Option<Duration>,
) -> Result<TcpStream, ConnectorError> {
    let Some(keep_alive) = keep_alive else {
        return Ok(stream);
    };
    let std_stream = stream.into_std().map_err(ConnectorError::SocketOptions)?;
    let socket = socket2::Socket::from(std_stream);
    let keep_alive = socket2::TcpKeepalive::new().with_time(keep_alive);
    socket
        .set_tcp_keepalive(&keep_alive)
        .map_err(ConnectorError::SocketOptions)?;
    socket
        .set_nonblocking(true)
        .map_err(ConnectorError::SocketOptions)?;
    TcpStream::from_std(socket.into()).map_err(ConnectorError::SocketOptions)
}

fn parse_local_address(local_address: Option<&str>) -> Result<Option<IpAddr>, ConnectorError> {
    local_address
        .map(|value| {
            value.parse::<IpAddr>().map_err(|error| {
                ConnectorError::SocketOptions(io::Error::new(io::ErrorKind::InvalidInput, error))
            })
        })
        .transpose()
}

async fn connect_socket(
    address: SocketAddr,
    local_address: Option<IpAddr>,
) -> Result<TcpStream, io::Error> {
    if local_address.is_some_and(|local| local.is_ipv4() != address.is_ipv4()) {
        return Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "local address family does not match remote address family",
        ));
    }
    let socket = if address.is_ipv4() {
        tokio::net::TcpSocket::new_v4()?
    } else {
        tokio::net::TcpSocket::new_v6()?
    };
    if let Some(local_address) = local_address {
        socket.bind(SocketAddr::new(local_address, 0))?;
    }
    socket.connect(address).await
}
