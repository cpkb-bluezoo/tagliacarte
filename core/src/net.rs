/*
 * net.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * This file is part of Tagliacarte, a cross-platform email client.
 *
 * Tagliacarte is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Tagliacarte is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Tagliacarte.  If not, see <http://www.gnu.org/licenses/>.
 */

//! TLS connection helpers: wrap TcpStream with rustls (implicit TLS, STARTTLS).
//!
//! Patterns follow gumdrop: Connection can be plain or secure; implicit TLS
//! handshakes immediately on connect; STARTTLS upgrades a plain stream after
//! protocol negotiation.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio_rustls::rustls::client::ClientConfig;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::RootCertStore;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream as TokioTlsStream;
use tokio_rustls::TlsConnector;

/// Build a root certificate store: platform native certs first, then webpki-roots as fallback.
fn build_root_store() -> RootCertStore {
    let mut root_store = RootCertStore::empty();
    match rustls_native_certs::load_native_certs() {
        Ok(certs) => {
            for cert in certs {
                let _ = root_store.add(cert);
            }
        }
        Err(_) => {}
    }
    if root_store.is_empty() {
        root_store.roots = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
    }
    root_store
}

/// Default TLS client config (native + Mozilla roots, no client auth).
fn default_client_config() -> Arc<ClientConfig> {
    let config = ClientConfig::builder()
        .with_root_certificates(build_root_store())
        .with_no_client_auth();
    Arc::new(config)
}

/// TLS client config for HTTP/1.1 + HTTP/2 with ALPN (h2, http/1.1). Used by the HTTP client.
pub fn http_client_config() -> Arc<ClientConfig> {
    let mut config = ClientConfig::builder()
        .with_root_certificates(build_root_store())
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Arc::new(config)
}

static DEFAULT_CONNECTOR: std::sync::OnceLock<TlsConnector> = std::sync::OnceLock::new();

fn default_connector() -> &'static TlsConnector {
    DEFAULT_CONNECTOR.get_or_init(|| TlsConnector::from(default_client_config()))
}

/// Async TLS stream (wraps tokio-rustls client TlsStream over TcpStream).
pub struct TlsStreamWrapper {
    inner: TokioTlsStream<TcpStream>,
}

impl TlsStreamWrapper {
    /// Connect with implicit TLS (e.g. IMAPS 993, SMTPS 465).
    /// TCP connect then immediate TLS handshake (gumdrop: secure == true path).
    pub async fn connect_implicit_tls(host: &str, port: u16) -> io::Result<Self> {
        let addr = format!("{}:{}", host, port);
        let tcp = TcpStream::connect(&addr).await?;
        let host_static: &'static str = Box::leak(host.to_string().into_boxed_str());
        let server_name: ServerName<'_> = host_static
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid host name"))?;
        let tls = default_connector()
            .connect(server_name, tcp)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;
        Ok(Self { inner: tls })
    }

    /// Access the underlying TLS stream (e.g. for splitting into reader/writer).
    pub fn inner(&self) -> &TokioTlsStream<TcpStream> {
        &self.inner
    }

    /// Consume and return the inner stream.
    pub fn into_inner(self) -> TokioTlsStream<TcpStream> {
        self.inner
    }
}

impl AsyncRead for TlsStreamWrapper {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for TlsStreamWrapper {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Plain TCP stream intended for STARTTLS upgrade (e.g. IMAP 143, SMTP 587).
/// Use `connect_plain` then protocol handshake, then `upgrade_to_tls` when the server supports STARTTLS.
pub struct PlainStream {
    inner: TcpStream,
}

impl PlainStream {
    /// Connect without TLS (for protocols that use STARTTLS).
    pub async fn connect(host: &str, port: u16) -> io::Result<Self> {
        let addr = format!("{}:{}", host, port);
        let tcp = TcpStream::connect(&addr).await?;
        Ok(Self { inner: tcp })
    }

    /// Upgrade this plain stream to TLS (after STARTTLS command accepted).
    /// Consumes `self` and returns a TLS stream using the same TCP connection.
    pub async fn upgrade_to_tls(self, host: &str) -> io::Result<TlsStreamWrapper> {
        let host_static: &'static str = Box::leak(host.to_string().into_boxed_str());
        let server_name: ServerName<'_> = host_static
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid host name"))?;
        let tls = default_connector()
            .connect(server_name, self.inner)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, e))?;
        Ok(TlsStreamWrapper { inner: tls })
    }

    pub fn inner(&self) -> &TcpStream {
        &self.inner
    }
}

impl AsyncRead for PlainStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for PlainStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Connect with implicit TLS (e.g. 993, 465).
/// Preserved name for API compatibility with the original stub.
pub async fn connect_implicit_tls(host: &str, port: u16) -> io::Result<TlsStreamWrapper> {
    TlsStreamWrapper::connect_implicit_tls(host, port).await
}

/// Connect plain (for STARTTLS). Returns a plain stream; call `PlainStream::upgrade_to_tls(host)` after the server agrees to STARTTLS.
pub async fn connect_plain(host: &str, port: u16) -> io::Result<PlainStream> {
    PlainStream::connect(host, port).await
}

/// Alias for STARTTLS flow: connect plain, then upgrade when the server supports it.
pub async fn connect_starttls(host: &str, port: u16) -> io::Result<PlainStream> {
    connect_plain(host, port).await
}

/// Type alias for the TLS stream (backward compatibility with stub that used the name TlsStream).
pub type TlsStream = TlsStreamWrapper;
