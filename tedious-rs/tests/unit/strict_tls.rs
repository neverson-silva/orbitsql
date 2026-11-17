use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tedious_rs::transport::{TransportError, connect_strict_tls};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

struct CertificateFiles {
    directory: PathBuf,
    certificate: Vec<u8>,
    key: Vec<u8>,
}

static CERTIFICATE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl Drop for CertificateFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn certificate_files() -> CertificateFiles {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sequence = CERTIFICATE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "tedious-rs-strict-tls-{}-{timestamp}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let key_pem = directory.join("key.pem");
    let certificate_pem = directory.join("certificate.pem");
    let certificate_der = directory.join("certificate.der");
    let key_der = directory.join("key.der");
    let status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost",
            "-keyout",
            key_pem.to_str().unwrap(),
            "-out",
            certificate_pem.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "openssl certificate generation failed"
    );
    let status = Command::new("openssl")
        .args([
            "x509",
            "-in",
            certificate_pem.to_str().unwrap(),
            "-outform",
            "der",
            "-out",
            certificate_der.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "openssl certificate conversion failed"
    );
    let status = Command::new("openssl")
        .args([
            "pkcs8",
            "-topk8",
            "-nocrypt",
            "-in",
            key_pem.to_str().unwrap(),
            "-outform",
            "der",
            "-out",
            key_der.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(status.status.success(), "openssl key conversion failed");
    CertificateFiles {
        directory,
        certificate: fs::read(certificate_der).unwrap(),
        key: fs::read(key_der).unwrap(),
    }
}

async fn start_tls_server(
    alpn: Vec<Vec<u8>>,
) -> (
    std::net::SocketAddr,
    tokio::task::JoinHandle<(Option<String>, Vec<u8>)>,
) {
    let files = certificate_files();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(files.certificate.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(files.key.clone())),
        )
        .unwrap();
    config.alpn_protocols = alpn;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let directory = files.directory.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut tls = acceptor.accept(stream).await.unwrap();
        let server_name = tls.get_ref().1.server_name().map(str::to_owned);
        let mut data = Vec::new();
        let _ = tls.read_to_end(&mut data).await;
        let _ = fs::remove_dir_all(directory);
        (server_name, data)
    });
    (address, task)
}

#[tokio::test]
async fn negotiates_strict_tls_with_sni_and_required_alpn() {
    let (address, server) = start_tls_server(vec![b"tds/8.0".to_vec()]).await;
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut tls = connect_strict_tls(stream, "localhost", true).await.unwrap();
    tls.write_all(b"tds8").await.unwrap();
    tls.shutdown().await.unwrap();
    let (server_name, data) = tokio::time::timeout(std::time::Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(server_name.as_deref(), Some("localhost"));
    assert_eq!(data, b"tds8");
}

#[tokio::test]
async fn accepts_a_tls_server_that_does_not_negotiate_tds_8_alpn() {
    let (address, server) = start_tls_server(Vec::new()).await;
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut tls = connect_strict_tls(stream, "localhost", true).await.unwrap();
    tls.shutdown().await.unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.0, Some("localhost".to_owned()));
}

#[tokio::test]
async fn rejects_an_untrusted_certificate_in_strict_tls() {
    let (address, server) = start_tls_server(vec![b"tds/8.0".to_vec()]).await;
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let error = connect_strict_tls(stream, "localhost", false)
        .await
        .unwrap_err();
    assert!(
        matches!(error, TransportError::InvalidPacket(message) if message.contains("TLS handshake failed"))
    );
    let _ = server.await;
}
