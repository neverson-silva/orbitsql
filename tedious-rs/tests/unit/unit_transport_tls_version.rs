#![cfg(not(target_vendor = "apple"))]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tedious_rs::transport::connect_tls;
use tokio::io::AsyncWriteExt;
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
        "tedious-rs-transport-tls-{}-{timestamp}-{sequence}",
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
    assert!(status.status.success());
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
    assert!(status.status.success());
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
    assert!(status.status.success());
    CertificateFiles {
        directory,
        certificate: fs::read(certificate_der).unwrap(),
        key: fs::read(key_der).unwrap(),
    }
}

#[tokio::test]
async fn native_tls_negotiates_tls13_when_server_supports_it() {
    let files = certificate_files();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let versions = &[&rustls::version::TLS13];
    let config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(versions)
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(files.certificate.clone())],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(files.key.clone())),
        )
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut tls = acceptor.accept(stream).await.unwrap();
        assert_eq!(
            tls.get_ref().1.protocol_version(),
            Some(rustls::ProtocolVersion::TLSv1_3)
        );
        tls.write_all(b"ok").await.unwrap();
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let mut tls = connect_tls(stream, "localhost", true).await.unwrap();
    let mut data = [0; 2];
    tokio::io::AsyncReadExt::read_exact(&mut tls, &mut data)
        .await
        .unwrap();
    assert_eq!(&data, b"ok");
    server.await.unwrap();
}
