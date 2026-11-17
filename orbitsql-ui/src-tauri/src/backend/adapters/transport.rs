use crate::backend::domain::connection::{
    ConnectionOperationError, ConnectionProfileRecord, ConnectionTransportMode, ProxyType,
};
use std::net::TcpListener as StdTcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

const TRANSPORT_TIMEOUT: Duration = Duration::from_secs(15);

pub struct PreparedEndpoint {
    pub host: String,
    pub port: u16,
    pub guard: Option<Arc<TransportGuard>>,
}

pub struct TransportGuard {
    inner: TransportGuardInner,
}

enum TransportGuardInner {
    Child(Arc<Mutex<Option<Child>>>),
    Proxy(Mutex<Option<oneshot::Sender<()>>>),
}

impl Drop for TransportGuard {
    fn drop(&mut self) {
        match &self.inner {
            TransportGuardInner::Child(child) => {
                if let Ok(mut child) = child.lock()
                    && let Some(mut child) = child.take()
                {
                    let _ = child.start_kill();
                }
            }
            TransportGuardInner::Proxy(shutdown) => {
                if let Ok(mut shutdown) = shutdown.lock()
                    && let Some(shutdown) = shutdown.take()
                {
                    let _ = shutdown.send(());
                }
            }
        }
    }
}

pub async fn prepare_endpoint(
    profile: &ConnectionProfileRecord,
) -> Result<PreparedEndpoint, ConnectionOperationError> {
    prepare_endpoint_for(&profile.server, profile.port, &profile.transport).await
}

pub async fn prepare_endpoint_for(
    server: &str,
    port: u16,
    transport: &crate::backend::domain::connection::ConnectionTransport,
) -> Result<PreparedEndpoint, ConnectionOperationError> {
    match transport.mode {
        ConnectionTransportMode::Direct => Ok(PreparedEndpoint {
            host: server.to_owned(),
            port,
            guard: None,
        }),
        ConnectionTransportMode::Ssh => prepare_ssh(server, port, transport).await,
        ConnectionTransportMode::Proxy => prepare_proxy(server, port, transport).await,
    }
}

async fn prepare_ssh(
    server: &str,
    port: u16,
    transport: &crate::backend::domain::connection::ConnectionTransport,
) -> Result<PreparedEndpoint, ConnectionOperationError> {
    if transport.ssh_host.trim().is_empty() || transport.ssh_user_name.trim().is_empty() {
        return Err(transport_error(
            "SSH host and SSH user are required for an SSH tunnel",
        ));
    }
    if transport.ssh_port == 0 {
        return Err(transport_error("SSH port must be greater than zero"));
    }
    let local_port = free_local_port()?;
    let mut command = Command::new("ssh");
    command
        .arg("-N")
        .arg("-T")
        .arg("-o")
        .arg("ExitOnForwardFailure=yes")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-L")
        .arg(format!("127.0.0.1:{local_port}:{}:{}", server, port))
        .arg("-p")
        .arg(transport.ssh_port.to_string());
    if !transport.ssh_private_key_path.trim().is_empty() {
        command.arg("-i").arg(&transport.ssh_private_key_path);
    }
    let child = command
        .arg(format!(
            "{}@{}",
            transport.ssh_user_name, transport.ssh_host
        ))
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| transport_error(format!("Unable to start SSH: {error}")))?;
    let child = Arc::new(Mutex::new(Some(child)));
    wait_for_endpoint("127.0.0.1", local_port, &child).await?;
    Ok(PreparedEndpoint {
        host: "127.0.0.1".to_owned(),
        port: local_port,
        guard: Some(Arc::new(TransportGuard {
            inner: TransportGuardInner::Child(child),
        })),
    })
}

async fn prepare_proxy(
    server: &str,
    port: u16,
    transport: &crate::backend::domain::connection::ConnectionTransport,
) -> Result<PreparedEndpoint, ConnectionOperationError> {
    if transport.proxy_host.trim().is_empty() || transport.proxy_port == 0 {
        return Err(transport_error("Proxy host and port are required"));
    }
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| transport_error(format!("Unable to create proxy endpoint: {error}")))?;
    let local_port = listener
        .local_addr()
        .map_err(|error| transport_error(format!("Unable to read proxy endpoint: {error}")))?
        .port();
    let (shutdown, shutdown_signal) = oneshot::channel();
    let proxy_host = transport.proxy_host.clone();
    let proxy_port = transport.proxy_port;
    let proxy_type = transport.proxy_type.clone();
    let target_host = server.to_owned();
    let target_port = port;
    tokio::spawn(run_proxy_listener(
        listener,
        proxy_host,
        proxy_port,
        proxy_type,
        target_host,
        target_port,
        shutdown_signal,
    ));
    Ok(PreparedEndpoint {
        host: "127.0.0.1".to_owned(),
        port: local_port,
        guard: Some(Arc::new(TransportGuard {
            inner: TransportGuardInner::Proxy(Mutex::new(Some(shutdown))),
        })),
    })
}

fn free_local_port() -> Result<u16, ConnectionOperationError> {
    let listener = StdTcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| transport_error(format!("Unable to allocate tunnel port: {error}")))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| transport_error(format!("Unable to read tunnel port: {error}")))
}

async fn wait_for_endpoint(
    host: &str,
    port: u16,
    child: &Arc<Mutex<Option<Child>>>,
) -> Result<(), ConnectionOperationError> {
    let started = tokio::time::Instant::now();
    loop {
        if let Ok(mut child) = child.lock()
            && let Some(child) = child.as_mut()
            && child
                .try_wait()
                .map_err(|error| transport_error(format!("SSH tunnel failed: {error}")))?
                .is_some()
        {
            return Err(transport_error("SSH tunnel exited before it became ready"));
        }
        if TcpStream::connect((host, port)).await.is_ok() {
            return Ok(());
        }
        if started.elapsed() >= TRANSPORT_TIMEOUT {
            return Err(transport_error("SSH tunnel startup timed out"));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn run_proxy_listener(
    listener: TcpListener,
    proxy_host: String,
    proxy_port: u16,
    proxy_type: ProxyType,
    target_host: String,
    target_port: u16,
    mut shutdown: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            result = listener.accept() => {
                let Ok((client, _)) = result else { break };
                let proxy_host = proxy_host.clone();
                let proxy_type = proxy_type.clone();
                let target_host = target_host.clone();
                tokio::spawn(async move {
                    let _ = relay_proxy(client, proxy_host, proxy_port, proxy_type, target_host, target_port).await;
                });
            }
        }
    }
}

async fn relay_proxy(
    mut client: TcpStream,
    proxy_host: String,
    proxy_port: u16,
    proxy_type: ProxyType,
    target_host: String,
    target_port: u16,
) -> Result<(), std::io::Error> {
    let mut upstream = TcpStream::connect((proxy_host.as_str(), proxy_port)).await?;
    match proxy_type {
        ProxyType::Socks5 => connect_socks5(&mut upstream, &target_host, target_port).await?,
        ProxyType::HttpConnect => connect_http(&mut upstream, &target_host, target_port).await?,
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
    Ok(())
}

async fn connect_socks5(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
) -> Result<(), std::io::Error> {
    stream.write_all(&[5, 1, 0]).await?;
    let mut response = [0; 2];
    stream.read_exact(&mut response).await?;
    if response != [5, 0] {
        return Err(std::io::Error::other(
            "SOCKS5 proxy requires unsupported authentication",
        ));
    }
    let host = host.as_bytes();
    if host.len() > u8::MAX as usize {
        return Err(std::io::Error::other("SOCKS5 target hostname is too long"));
    }
    let mut request = vec![5, 1, 0, 3, host.len() as u8];
    request.extend_from_slice(host);
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await?;
    read_proxy_response(stream).await
}

async fn connect_http(stream: &mut TcpStream, host: &str, port: u16) -> Result<(), std::io::Error> {
    stream
        .write_all(
            format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n").as_bytes(),
        )
        .await?;
    let mut response = Vec::new();
    let mut byte = [0; 1];
    while response.len() < 8192 {
        stream.read_exact(&mut byte).await?;
        response.push(byte[0]);
        if response.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let status = String::from_utf8_lossy(&response);
    if status.starts_with("HTTP/") && status.split_whitespace().nth(1) == Some("200") {
        Ok(())
    } else {
        Err(std::io::Error::other("HTTP proxy refused CONNECT"))
    }
}

async fn read_proxy_response(stream: &mut TcpStream) -> Result<(), std::io::Error> {
    let mut header = [0; 4];
    stream.read_exact(&mut header).await?;
    if header[1] != 0 {
        return Err(std::io::Error::other("Proxy refused database connection"));
    }
    let length = match header[3] {
        1 => 4,
        3 => {
            let mut size = [0; 1];
            stream.read_exact(&mut size).await?;
            usize::from(size[0])
        }
        4 => 16,
        _ => {
            return Err(std::io::Error::other(
                "Proxy returned an invalid address type",
            ));
        }
    };
    let mut address = vec![0; length + 2];
    stream.read_exact(&mut address).await?;
    Ok(())
}

fn transport_error(message: impl Into<String>) -> ConnectionOperationError {
    ConnectionOperationError::new("TRANSPORT_FAILED", message, true)
}
