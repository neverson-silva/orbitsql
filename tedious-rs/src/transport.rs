use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures_util::{Stream, StreamExt, stream};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as RustlsError, RootCertStore, SignatureScheme,
};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::time::timeout;
use tokio_native_tls::{TlsConnector, TlsStream, native_tls};
use tokio_rustls::TlsConnector as RustlsConnector;
use tokio_rustls::client::TlsStream as RustlsTlsStream;

use crate::bulk_load::BulkLoadCancellationHandle;
use crate::connector::{
    AddressLookup, CancellationToken, SystemAddressLookup, lookup_all_addresses_with,
};
use crate::debug::DebugOptions;
use crate::message::Message;
use crate::packet::{HEADER_LENGTH, Packet, packet_type};

#[derive(Debug)]
pub enum TransportError {
    Io(io::Error),
    Timeout,
    InvalidPacket(String),
}

#[derive(Clone, PartialEq, Eq)]
pub struct IncomingMessageChunk {
    pub message_type: u8,
    pub reset_connection: bool,
    pub ignore: bool,
    pub data: Vec<u8>,
    pub end_of_message: bool,
}

impl std::fmt::Debug for IncomingMessageChunk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IncomingMessageChunk")
            .field("message_type", &self.message_type)
            .field("reset_connection", &self.reset_connection)
            .field("ignore", &self.ignore)
            .field(
                "data",
                &format_args!("[REDACTED; {} bytes]", self.data.len()),
            )
            .field("end_of_message", &self.end_of_message)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IncomingMessageChunkResult {
    Chunk(IncomingMessageChunk),
    EndOfStream,
    Paused,
}

#[derive(Clone)]
pub struct IncomingMessagePauseHandle {
    paused: Arc<AtomicBool>,
    resume: Arc<Notify>,
}

impl IncomingMessagePauseHandle {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
        self.resume.notify_waiters();
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }
}

impl From<io::Error> for TransportError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub async fn connect_tcp(
    host: &str,
    port: u16,
    connect_timeout: Duration,
) -> Result<TcpStream, TransportError> {
    connect_tcp_with_lookup(
        host,
        port,
        connect_timeout,
        &SystemAddressLookup,
        &CancellationToken::new(),
    )
    .await
}

pub async fn connect_tcp_with_lookup(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    lookup: &dyn AddressLookup,
    cancellation: &CancellationToken,
) -> Result<TcpStream, TransportError> {
    let deadline = Instant::now() + connect_timeout;
    let addresses = match timeout(
        connect_timeout,
        lookup_all_addresses_with(host, port, lookup, cancellation),
    )
    .await
    {
        Ok(result) => result.map_err(|error| TransportError::InvalidPacket(error.to_string()))?,
        Err(_) => return Err(TransportError::Timeout),
    };
    if addresses.is_empty() {
        return Err(TransportError::InvalidPacket(
            "host resolved to no addresses".to_owned(),
        ));
    }
    let mut errors = Vec::new();
    for address in addresses {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err(TransportError::Timeout);
        };
        let result = tokio::select! {
            result = timeout(remaining, TcpStream::connect(address)) => result,
            _ = cancellation.cancelled() => return Err(TransportError::Timeout),
        };
        match result {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => errors.push(error.to_string()),
            Err(_) => errors.push("connection timeout".to_owned()),
        }
    }
    Err(TransportError::InvalidPacket(format!(
        "could not connect: {}",
        errors.join("; ")
    )))
}

pub async fn connect_tls(
    stream: TcpStream,
    hostname: &str,
    trust_server_certificate: bool,
) -> Result<TlsStream<TcpStream>, TransportError> {
    let mut builder = native_tls::TlsConnector::builder();
    builder.danger_accept_invalid_certs(trust_server_certificate);
    builder.danger_accept_invalid_hostnames(trust_server_certificate);
    let connector = TlsConnector::from(builder.build().map_err(|error| {
        TransportError::InvalidPacket(format!("TLS configuration failed: {error}"))
    })?);
    connector
        .connect(hostname, stream)
        .await
        .map_err(|error| TransportError::InvalidPacket(format!("TLS handshake failed: {error}")))
}

struct TrustAnyCertificate {
    supported: rustls::crypto::WebPkiSupportedAlgorithms,
}

impl std::fmt::Debug for TrustAnyCertificate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TrustAnyCertificate")
    }
}

impl ServerCertVerifier for TrustAnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        rustls::crypto::verify_tls12_signature(_message, _cert, _dss, &self.supported)
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        rustls::crypto::verify_tls13_signature(_message, _cert, _dss, &self.supported)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.supported.supported_schemes()
    }
}

pub async fn connect_strict_tls(
    stream: TcpStream,
    hostname: &str,
    trust_server_certificate: bool,
) -> Result<RustlsTlsStream<TcpStream>, TransportError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let supported = provider.signature_verification_algorithms;
    let mut config = if trust_server_certificate {
        let mut config = ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(|error| {
                TransportError::InvalidPacket(format!("TLS configuration failed: {error}"))
            })?
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth();
        config
            .dangerous()
            .set_certificate_verifier(Arc::new(TrustAnyCertificate { supported }));
        config
    } else {
        let native = rustls_native_certs::load_native_certs();
        if !native.errors.is_empty() {
            return Err(TransportError::InvalidPacket(format!(
                "native certificate loading failed: {}",
                native
                    .errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        let mut roots = RootCertStore::empty();
        for certificate in native.certs {
            roots.add(certificate).map_err(|error| {
                TransportError::InvalidPacket(format!("native certificate rejected: {error}"))
            })?;
        }
        ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|error| {
                TransportError::InvalidPacket(format!("TLS configuration failed: {error}"))
            })?
            .with_root_certificates(roots)
            .with_no_client_auth()
    };
    config.alpn_protocols = vec![b"tds/8.0".to_vec()];
    let name = ServerName::try_from(hostname.to_owned()).map_err(|error| {
        TransportError::InvalidPacket(format!("invalid TLS server name: {error}"))
    })?;
    let connector = RustlsConnector::from(Arc::new(config));
    let tls = connector
        .connect(name, stream)
        .await
        .map_err(|error| TransportError::InvalidPacket(format!("TLS handshake failed: {error}")))?;
    Ok(tls)
}

pub async fn connect_tds_tls(
    stream: TcpStream,
    hostname: &str,
    trust_server_certificate: bool,
    packet_size: usize,
) -> Result<TlsStream<TdsTlsStream<TcpStream>>, TransportError> {
    let mut builder = native_tls::TlsConnector::builder();
    builder.danger_accept_invalid_certs(trust_server_certificate);
    builder.danger_accept_invalid_hostnames(trust_server_certificate);
    builder.max_protocol_version(Some(native_tls::Protocol::Tlsv12));
    let connector = TlsConnector::from(builder.build().map_err(|error| {
        TransportError::InvalidPacket(format!("TLS configuration failed: {error}"))
    })?);
    let tls_transport = TdsTlsStream::new(stream, packet_size)?;
    let mut tls = connector
        .connect(hostname, tls_transport)
        .await
        .map_err(|error| TransportError::InvalidPacket(format!("TLS handshake failed: {error}")))?;
    tls.get_mut()
        .get_mut()
        .get_mut()
        .mark_tls_negotiation_complete();
    Ok(tls)
}

pub struct TdsTlsStream<S> {
    inner: S,
    packet_size: usize,
    incoming: Vec<u8>,
    incoming_offset: usize,
    wire: Vec<u8>,
    pending_write: Vec<u8>,
    pending_write_offset: usize,
    pending_write_accepted: usize,
    packet_id: u8,
    tls_negotiation_complete: bool,
    handshake_write_buffer: Vec<u8>,
    handshake_write_offset: usize,
}

impl<S> TdsTlsStream<S> {
    pub fn new(inner: S, packet_size: usize) -> Result<Self, TransportError> {
        if packet_size <= HEADER_LENGTH || packet_size > u16::MAX as usize {
            return Err(TransportError::InvalidPacket(
                "invalid packet size".to_owned(),
            ));
        }
        Ok(Self {
            inner,
            packet_size,
            incoming: Vec::new(),
            incoming_offset: 0,
            wire: Vec::new(),
            pending_write: Vec::new(),
            pending_write_offset: 0,
            pending_write_accepted: 0,
            packet_id: 1,
            tls_negotiation_complete: false,
            handshake_write_buffer: Vec::new(),
            handshake_write_offset: 0,
        })
    }

    pub fn set_packet_size(&mut self, packet_size: usize) -> Result<(), TransportError> {
        if packet_size <= HEADER_LENGTH || packet_size > u16::MAX as usize {
            return Err(TransportError::InvalidPacket(
                "invalid packet size".to_owned(),
            ));
        }
        self.packet_size = packet_size;
        Ok(())
    }

    fn available(&self) -> usize {
        self.incoming.len().saturating_sub(self.incoming_offset)
    }

    fn mark_tls_negotiation_complete(&mut self) {
        self.tls_negotiation_complete = true;
    }

    fn compact_incoming(&mut self) {
        if self.incoming_offset > 0 {
            self.incoming.drain(..self.incoming_offset);
            self.incoming_offset = 0;
        }
    }

    fn decode_packets(&mut self) -> Result<(), TransportError> {
        loop {
            if self.wire.len() < HEADER_LENGTH {
                return Ok(());
            }
            let length = u16::from_be_bytes([self.wire[2], self.wire[3]]) as usize;
            if length < HEADER_LENGTH {
                return Err(TransportError::InvalidPacket(
                    "invalid TDS TLS packet length".to_owned(),
                ));
            }
            if self.wire.len() < length {
                return Ok(());
            }
            let payload = self.wire[HEADER_LENGTH..length].to_vec();
            self.wire.drain(..length);
            self.compact_incoming();
            self.incoming.extend_from_slice(&payload);
        }
    }

    fn prepare_write(&mut self, data: &[u8]) -> usize {
        let chunk_length = data.len().min(self.packet_size - HEADER_LENGTH);
        self.pending_write = vec![0; HEADER_LENGTH + chunk_length];
        self.pending_write[0] = packet_type::PRELOGIN;
        if chunk_length == data.len() {
            self.pending_write[1] = crate::packet::status::EOM;
        }
        let packet_length = self.pending_write.len() as u16;
        self.pending_write[2..4].copy_from_slice(&packet_length.to_be_bytes());
        self.pending_write[6] = self.packet_id;
        self.pending_write[HEADER_LENGTH..].copy_from_slice(&data[..chunk_length]);
        self.pending_write_offset = 0;
        self.pending_write_accepted = chunk_length;
        chunk_length
    }
}

impl<S> AsyncRead for TdsTlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if !self.tls_negotiation_complete && !self.handshake_write_buffer.is_empty() {
            loop {
                if self.pending_write.is_empty() {
                    let start = self.handshake_write_offset;
                    let handshake_data = self.handshake_write_buffer[start..].to_vec();
                    let accepted = self.prepare_write(&handshake_data);
                    if accepted == 0 {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "TDS TLS handshake produced an empty write",
                        )));
                    }
                }
                let offset = self.pending_write_offset;
                let pending = self.pending_write[offset..].to_vec();
                match Pin::new(&mut self.inner).poll_write(cx, &pending) {
                    Poll::Ready(Ok(0)) => {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "TDS TLS handshake wrote zero bytes",
                        )));
                    }
                    Poll::Ready(Ok(written)) => {
                        self.pending_write_offset += written;
                        if self.pending_write_offset == self.pending_write.len() {
                            let accepted = self.pending_write_accepted;
                            self.pending_write.clear();
                            self.pending_write_offset = 0;
                            self.pending_write_accepted = 0;
                            self.handshake_write_offset += accepted;
                            self.packet_id = self.packet_id.wrapping_add(1);
                            if self.handshake_write_offset == self.handshake_write_buffer.len() {
                                self.handshake_write_buffer.clear();
                                self.handshake_write_offset = 0;
                                break;
                            }
                        }
                    }
                    Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
        if self.available() > 0 {
            let length = self.available().min(output.remaining());
            let start = self.incoming_offset;
            output.put_slice(&self.incoming[start..start + length]);
            self.incoming_offset += length;
            if self.incoming_offset == self.incoming.len() {
                self.incoming.clear();
                self.incoming_offset = 0;
            }
            return Poll::Ready(Ok(()));
        }
        if self.tls_negotiation_complete {
            return Pin::new(&mut self.inner).poll_read(cx, output);
        }
        loop {
            if self.available() > 0 {
                let length = self.available().min(output.remaining());
                let start = self.incoming_offset;
                output.put_slice(&self.incoming[start..start + length]);
                self.incoming_offset += length;
                if self.incoming_offset == self.incoming.len() {
                    self.incoming.clear();
                    self.incoming_offset = 0;
                }
                return Poll::Ready(Ok(()));
            }
            if let Err(error) = self.decode_packets() {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    error_string(error),
                )));
            }
            if self.available() > 0 {
                continue;
            }
            let mut buffer = [0u8; 8192];
            let mut read_buf = ReadBuf::new(&mut buffer);
            match Pin::new(&mut self.inner).poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => {
                    if read_buf.filled().is_empty() {
                        if !self.wire.is_empty() {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "TDS TLS packet ended before its declared length",
                            )));
                        }
                        return Poll::Ready(Ok(()));
                    }
                    self.wire.extend_from_slice(read_buf.filled());
                }
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<S> AsyncWrite for TdsTlsStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.tls_negotiation_complete {
            return Pin::new(&mut self.inner).poll_write(cx, data);
        }
        if self.handshake_write_buffer.is_empty() && self.pending_write.is_empty() {
            self.packet_id = 1;
        }
        self.handshake_write_buffer.extend_from_slice(data);
        Poll::Ready(Ok(data.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if !self.tls_negotiation_complete {
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

fn error_string(error: TransportError) -> String {
    match error {
        TransportError::Io(value) => value.to_string(),
        TransportError::Timeout => "transport timeout".to_owned(),
        TransportError::InvalidPacket(value) => value,
    }
}

pub struct MessageIo<S> {
    stream: S,
    packet_size: usize,
    debug_options: Option<DebugOptions>,
    debug_listener: Option<Arc<dyn Fn(String) + Send + Sync>>,
    incoming_message_type: Option<u8>,
    incoming_reset_connection: bool,
    incoming_ignore: bool,
    incoming_paused: Arc<AtomicBool>,
    incoming_resume: Arc<Notify>,
    incoming_terminal: bool,
}

impl<S> MessageIo<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S, packet_size: usize) -> Result<Self, TransportError> {
        if packet_size <= HEADER_LENGTH || packet_size > u16::MAX as usize {
            return Err(TransportError::InvalidPacket(
                "invalid packet size".to_owned(),
            ));
        }
        Ok(Self {
            stream,
            packet_size,
            debug_options: None,
            debug_listener: None,
            incoming_message_type: None,
            incoming_reset_connection: false,
            incoming_ignore: false,
            incoming_paused: Arc::new(AtomicBool::new(false)),
            incoming_resume: Arc::new(Notify::new()),
            incoming_terminal: false,
        })
    }

    pub fn set_debug(
        &mut self,
        options: DebugOptions,
        listener: Arc<dyn Fn(String) + Send + Sync>,
    ) {
        self.debug_options = Some(options);
        self.debug_listener = Some(listener);
    }

    pub fn packet_size(&self) -> usize {
        self.packet_size
    }
    pub fn set_packet_size(&mut self, packet_size: usize) -> Result<(), TransportError> {
        if packet_size <= HEADER_LENGTH || packet_size > u16::MAX as usize {
            return Err(TransportError::InvalidPacket(
                "invalid packet size".to_owned(),
            ));
        }
        self.packet_size = packet_size;
        Ok(())
    }

    pub fn stream_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    pub fn pause_incoming(&self) {
        self.incoming_paused.store(true, Ordering::Release);
    }

    pub fn resume_incoming(&self) {
        self.incoming_paused.store(false, Ordering::Release);
        self.incoming_resume.notify_waiters();
    }

    pub fn incoming_is_paused(&self) -> bool {
        self.incoming_paused.load(Ordering::Acquire)
    }

    pub fn incoming_pause_handle(&self) -> IncomingMessagePauseHandle {
        IncomingMessagePauseHandle {
            paused: Arc::clone(&self.incoming_paused),
            resume: Arc::clone(&self.incoming_resume),
        }
    }

    pub async fn send_message(&mut self, message: &Message) -> Result<(), TransportError> {
        let packets = crate::message::packetize(message, self.packet_size)
            .map_err(|error| TransportError::InvalidPacket(error.0))?;
        for packet in packets {
            self.debug_packet("Sent", &packet);
            self.stream.write_all(&packet.buffer).await?;
        }
        self.stream.flush().await?;
        Ok(())
    }

    pub async fn send_message_buffers(
        &mut self,
        message_type: u8,
        buffers: &[Vec<u8>],
    ) -> Result<(), TransportError> {
        self.send_message_buffers_with_flags(message_type, false, buffers)
            .await
    }

    pub async fn send_message_buffers_with_flags(
        &mut self,
        message_type: u8,
        reset_connection: bool,
        buffers: &[Vec<u8>],
    ) -> Result<(), TransportError> {
        let payload_size = self.packet_size - HEADER_LENGTH;
        let mut payload = Vec::with_capacity(payload_size);
        let mut packet_id = 1u8;
        let mut emitted = false;

        for buffer in buffers {
            let mut offset = 0;
            while offset < buffer.len() {
                if payload.len() == payload_size {
                    let packet =
                        stream_packet(message_type, packet_id, false, reset_connection, &payload)?;
                    self.debug_packet("Sent", &packet);
                    self.stream.write_all(&packet.buffer).await?;
                    packet_id = packet_id.wrapping_add(1);
                    payload.clear();
                    emitted = true;
                }
                let available = payload_size - payload.len();
                let length = available.min(buffer.len() - offset);
                payload.extend_from_slice(&buffer[offset..offset + length]);
                offset += length;
            }
        }

        if !payload.is_empty() || !emitted {
            let packet = stream_packet(message_type, packet_id, true, reset_connection, &payload)?;
            self.debug_packet("Sent", &packet);
            self.stream.write_all(&packet.buffer).await?;
        }
        self.stream.flush().await?;
        Ok(())
    }

    pub async fn send_message_parts<I>(
        &mut self,
        message_type: u8,
        parts: I,
    ) -> Result<(), TransportError>
    where
        I: IntoIterator<Item = Result<Vec<u8>, TransportError>>,
    {
        let payload_size = self.packet_size - HEADER_LENGTH;
        let mut payload = Vec::with_capacity(payload_size);
        let mut packet_id = 1u8;
        let mut emitted = false;
        for part in parts {
            let part = part?;
            let mut offset = 0;
            while offset < part.len() {
                if payload.len() == payload_size {
                    let packet = stream_packet(message_type, packet_id, false, false, &payload)?;
                    self.debug_packet("Sent", &packet);
                    self.stream.write_all(&packet.buffer).await?;
                    packet_id = packet_id.wrapping_add(1);
                    payload.clear();
                    emitted = true;
                }
                let length = (payload_size - payload.len()).min(part.len() - offset);
                payload.extend_from_slice(&part[offset..offset + length]);
                offset += length;
            }
        }
        if !payload.is_empty() || !emitted {
            let packet = stream_packet(message_type, packet_id, true, false, &payload)?;
            self.debug_packet("Sent", &packet);
            self.stream.write_all(&packet.buffer).await?;
        }
        self.stream.flush().await?;
        Ok(())
    }

    pub async fn send_message_stream<P>(
        &mut self,
        message_type: u8,
        parts: P,
    ) -> Result<(), TransportError>
    where
        P: Stream<Item = Result<Vec<u8>, TransportError>>,
    {
        let mut parts = Box::pin(parts);
        let payload_size = self.packet_size - HEADER_LENGTH;
        let mut payload = Vec::with_capacity(payload_size);
        let mut packet_id = 1u8;
        let mut emitted = false;
        while let Some(part) = parts.next().await {
            let part = part?;
            let mut offset = 0;
            while offset < part.len() {
                if payload.len() == payload_size {
                    let packet = stream_packet(message_type, packet_id, false, false, &payload)?;
                    self.debug_packet("Sent", &packet);
                    self.stream.write_all(&packet.buffer).await?;
                    packet_id = packet_id.wrapping_add(1);
                    payload.clear();
                    emitted = true;
                }
                let length = (payload_size - payload.len()).min(part.len() - offset);
                payload.extend_from_slice(&part[offset..offset + length]);
                offset += length;
            }
        }
        if !payload.is_empty() || !emitted {
            let packet = stream_packet(message_type, packet_id, true, false, &payload)?;
            self.debug_packet("Sent", &packet);
            self.stream.write_all(&packet.buffer).await?;
        }
        self.stream.flush().await?;
        Ok(())
    }

    pub async fn send_message_stream_with_cancellation<P>(
        &mut self,
        message_type: u8,
        parts: P,
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<bool, TransportError>
    where
        P: Stream<Item = Result<Vec<u8>, TransportError>>,
    {
        let mut parts = Box::pin(parts);
        let payload_size = self.packet_size - HEADER_LENGTH;
        let mut payload = Vec::with_capacity(payload_size);
        let mut packet_id = 1u8;
        let mut emitted = false;
        let mut ignored = false;
        let mut producer_error = None;
        loop {
            if cancellation.is_canceled() {
                ignored = true;
                break;
            }
            let next = tokio::select! {
                part = parts.next() => part,
                _ = cancellation.canceled() => {
                    ignored = true;
                    break;
                }
            };
            let Some(part) = next else {
                break;
            };
            let part = match part {
                Ok(part) => part,
                Err(error) => {
                    ignored = true;
                    producer_error = Some(error);
                    break;
                }
            };
            let mut offset = 0;
            while offset < part.len() {
                if cancellation.is_canceled() {
                    ignored = true;
                    break;
                }
                if payload.len() == payload_size {
                    let packet = stream_packet(message_type, packet_id, false, false, &payload)?;
                    self.debug_packet("Sent", &packet);
                    self.stream.write_all(&packet.buffer).await?;
                    packet_id = packet_id.wrapping_add(1);
                    payload.clear();
                    emitted = true;
                }
                let length = (payload_size - payload.len()).min(part.len() - offset);
                payload.extend_from_slice(&part[offset..offset + length]);
                offset += length;
            }
            if ignored {
                break;
            }
        }
        let finalization = if !payload.is_empty() || !emitted || ignored {
            let mut packet = stream_packet(message_type, packet_id, true, false, &payload)?;
            packet.ignore(ignored);
            self.debug_packet("Sent", &packet);
            self.stream.write_all(&packet.buffer).await
        } else {
            Ok(())
        };
        let flush = self.stream.flush().await;
        if let Some(error) = producer_error {
            return Err(error);
        }
        finalization?;
        flush?;
        Ok(ignored)
    }

    pub async fn send_message_buffers_with_cancellation(
        &mut self,
        message_type: u8,
        buffers: &[Vec<u8>],
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<bool, TransportError> {
        let parts = stream::iter(buffers.iter().cloned().map(Ok));
        self.send_message_stream_with_cancellation(message_type, parts, cancellation)
            .await
    }

    pub async fn send_message_parts_with_cancellation<I>(
        &mut self,
        message_type: u8,
        parts: I,
        cancellation: &BulkLoadCancellationHandle,
    ) -> Result<bool, TransportError>
    where
        I: IntoIterator<Item = Result<Vec<u8>, TransportError>>,
    {
        let parts = stream::iter(parts);
        self.send_message_stream_with_cancellation(message_type, parts, cancellation)
            .await
    }

    pub async fn send_attention(&mut self) -> Result<(), TransportError> {
        self.send_message(&Message::new(packet_type::ATTENTION))
            .await
    }

    pub async fn read_message(&mut self) -> Result<Message, TransportError> {
        let mut message: Option<Message> = None;
        loop {
            match self.next_message_chunk().await? {
                IncomingMessageChunkResult::Chunk(chunk) => {
                    let current = message.get_or_insert_with(|| Message::new(chunk.message_type));
                    current.reset_connection = chunk.reset_connection;
                    current.ignore = chunk.ignore;
                    current.data.extend_from_slice(&chunk.data);
                    if chunk.end_of_message {
                        return message.take().ok_or_else(|| {
                            TransportError::InvalidPacket(
                                "message ended without a payload".to_owned(),
                            )
                        });
                    }
                }
                IncomingMessageChunkResult::EndOfStream => {
                    return Err(TransportError::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "unexpected end of message stream",
                    )));
                }
                IncomingMessageChunkResult::Paused => {
                    self.wait_until_incoming_resumed().await;
                }
            }
        }
    }

    pub async fn next_message_chunk(
        &mut self,
    ) -> Result<IncomingMessageChunkResult, TransportError> {
        if self.incoming_paused.load(Ordering::Acquire) {
            return Ok(IncomingMessageChunkResult::Paused);
        }
        if self.incoming_terminal {
            return Ok(IncomingMessageChunkResult::EndOfStream);
        }

        let Some(packet) = self.read_incoming_packet().await? else {
            if self.incoming_message_type.is_some() {
                self.incoming_terminal = true;
                return Err(TransportError::Io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "incoming message ended before EOM",
                )));
            }
            self.incoming_terminal = true;
            return Ok(IncomingMessageChunkResult::EndOfStream);
        };

        let first_packet = self.incoming_message_type.is_none();
        if first_packet {
            self.incoming_message_type = Some(packet.packet_type());
            self.incoming_reset_connection =
                packet.status() & crate::packet::status::RESET_CONNECTION != 0;
            self.incoming_ignore = packet.status() & crate::packet::status::IGNORE != 0;
        }
        let end_of_message = packet.status() & crate::packet::status::EOM != 0;
        let message_type = match self.incoming_message_type {
            Some(message_type) => message_type,
            None => packet.packet_type(),
        };
        let chunk = IncomingMessageChunk {
            message_type,
            reset_connection: self.incoming_reset_connection,
            ignore: self.incoming_ignore,
            data: packet.data().to_vec(),
            end_of_message,
        };
        if end_of_message {
            self.incoming_message_type = None;
            self.incoming_reset_connection = false;
            self.incoming_ignore = false;
        }
        Ok(IncomingMessageChunkResult::Chunk(chunk))
    }

    pub async fn read_message_with_packet_callback<F>(
        &mut self,
        mut packet_callback: F,
    ) -> Result<Message, TransportError>
    where
        F: FnMut(&Packet),
    {
        let mut message: Option<Message> = None;
        loop {
            self.wait_until_incoming_resumed().await;
            let packet = self.read_incoming_packet().await?.ok_or_else(|| {
                TransportError::Io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "unexpected end of message stream",
                ))
            })?;
            packet_callback(&packet);
            let current = message.get_or_insert_with(|| Message::new(packet.packet_type()));
            current.reset_connection =
                packet.status() & crate::packet::status::RESET_CONNECTION != 0;
            current.ignore = packet.status() & crate::packet::status::IGNORE != 0;
            current.data.extend_from_slice(packet.data());
            if packet.status() & crate::packet::status::EOM != 0 {
                return message.take().ok_or_else(|| {
                    TransportError::InvalidPacket("message ended without a payload".to_owned())
                });
            }
        }
    }

    async fn wait_until_incoming_resumed(&self) {
        loop {
            let resumed = self.incoming_resume.notified();
            if !self.incoming_paused.load(Ordering::Acquire) {
                return;
            }
            resumed.await;
        }
    }

    async fn read_incoming_packet(&mut self) -> Result<Option<Packet>, TransportError> {
        let mut first = [0u8; 1];
        let read = match self.stream.read(&mut first).await {
            Ok(read) => read,
            Err(error) => {
                self.incoming_terminal = true;
                return Err(error.into());
            }
        };
        if read == 0 {
            return Ok(None);
        }
        let mut header_tail = [0u8; HEADER_LENGTH - 1];
        if let Err(error) = self.stream.read_exact(&mut header_tail).await {
            self.incoming_terminal = true;
            return Err(TransportError::Io(error));
        }
        let mut header = [0u8; HEADER_LENGTH];
        header[0] = first[0];
        header[1..].copy_from_slice(&header_tail);
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        if length < HEADER_LENGTH {
            self.incoming_terminal = true;
            return Err(TransportError::InvalidPacket(
                "received packet shorter than header".to_owned(),
            ));
        }
        let mut data = vec![0u8; length - HEADER_LENGTH];
        if let Err(error) = self.stream.read_exact(&mut data).await {
            self.incoming_terminal = true;
            return Err(TransportError::Io(error));
        }
        let packet = Packet::from_buffer([header.to_vec(), data].concat())
            .map_err(TransportError::InvalidPacket)?;
        self.debug_packet("Received", &packet);
        Ok(Some(packet))
    }

    pub fn into_inner(self) -> S {
        self.stream
    }

    fn debug_packet(&self, direction: &str, packet: &Packet) {
        let (Some(options), Some(listener)) = (&self.debug_options, &self.debug_listener) else {
            return;
        };
        if options.packet {
            listener(format!("\n{direction}\n{}", packet.header_to_string("  ")));
        }
        if options.data {
            listener("  [REDACTED PACKET DATA]".to_owned());
        }
    }
}

fn stream_packet(
    message_type: u8,
    packet_id: u8,
    last: bool,
    reset_connection: bool,
    data: &[u8],
) -> Result<Packet, TransportError> {
    let mut packet = Packet::new(message_type);
    packet.buffer[6] = packet_id;
    packet.last(Some(last));
    packet.reset_connection(reset_connection);
    packet
        .add_data(data)
        .map_err(TransportError::InvalidPacket)?;
    Ok(packet)
}

pub fn default_packet_type() -> u8 {
    packet_type::SQL_BATCH
}
