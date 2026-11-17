use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tedious_rs::packet::packet_type;
use tedious_rs::transport::{TdsTlsStream, connect_tds_tls};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

struct CertificateFiles {
    directory: PathBuf,
    certificate: Vec<u8>,
    key: Vec<u8>,
}

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
    let directory = std::env::temp_dir().join(format!(
        "tedious-rs-transport-tls-parity-{}-{timestamp}",
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

fn packet(data: &[u8]) -> Vec<u8> {
    let length = (8 + data.len()) as u16;
    let mut packet = vec![packet_type::TABULAR_RESULT, 1, 0, 0, 0, 0, 1, 0];
    packet[2..4].copy_from_slice(&length.to_be_bytes());
    packet.extend_from_slice(data);
    packet
}

#[tokio::test]
async fn accepts_tds_tls_packet_longer_than_configured_packet_size() {
    let (client, mut server) = duplex(128);
    let mut stream = TdsTlsStream::new(client, 16).unwrap();
    server.write_all(&packet(b"oversized")).await.unwrap();
    let mut data = [0; 9];
    stream.read_exact(&mut data).await.unwrap();
    assert_eq!(&data, b"oversized");
}

#[tokio::test]
async fn limits_tds_tls_to_tls12() {
    let files = certificate_files();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
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
        let framed = TdsTlsStream::new(stream, 4096).unwrap();
        acceptor.accept(framed).await.is_err()
    });
    let stream = tokio::net::TcpStream::connect(address).await.unwrap();
    assert!(
        connect_tds_tls(stream, "localhost", true, 4096)
            .await
            .is_err()
    );
    assert!(server.await.unwrap());
}
