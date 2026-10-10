/*
 * mtls.rs
 * Copyright (C) 2026 Chris Burdess
 *
 * Ephemeral CA + server + client certs for loopback mTLS (mail body WebView).
 */

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::Bytes;
use hopf_core::{
    acceptor_from_credentials, acceptor_from_credentials_with_client_auth,
    server_credentials_from_pem_bytes, ClientAuthPolicy, SharedTlsAcceptor,
};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType,
};

/// The loopback server speaks HTTP/1.1 only: the WebView streams one body
/// per request and gains nothing from HTTP/2 here.
const ALPN: &[&[u8]] = &[b"http/1.1"];

static REQUIRE_CLIENT_CERT: AtomicBool = AtomicBool::new(true);

/// When **true** (default), [`MtlsMaterial::generate`] uses mutual TLS. Set to **false** before
/// starting the mail-body HTTPS server if the embedded WebView cannot present a client certificate
/// yet (TLS to loopback is still used).
pub fn set_mail_body_tls_require_client_cert(require: bool) {
    REQUIRE_CLIENT_CERT.store(require, Ordering::Relaxed);
}

/// Current policy for [`MtlsMaterial::generate`] (before env override).
pub fn mail_body_tls_requires_client_cert() -> bool {
    REQUIRE_CLIENT_CERT.load(Ordering::Relaxed)
}

/// PEM strings and the hopf TLS acceptor for the local mail-body HTTPS server.
#[derive(Clone)]
pub struct MtlsMaterial {
    /// CA certificate (PEM) — WebView should trust this for server validation.
    pub ca_cert_pem: String,
    /// Client certificate + private key (PEM, concatenated cert then key) for WebView mTLS.
    pub client_cert_pem: String,
    pub client_key_pem: String,
    pub tls_acceptor: SharedTlsAcceptor,
    /// Whether accepted connections must present a valid client certificate.
    pub enforces_client_cert: bool,
}

impl MtlsMaterial {
    /// Generate a fresh CA, server cert for `localhost` / `127.0.0.1`, and client cert. All in memory.
    ///
    /// Uses **mutual TLS** when [`mail_body_tls_requires_client_cert`] is true (default) and
    /// `TAGLIACARTE_MAIL_BODY_TLS_NO_CLIENT_CERT` is not set. Otherwise TLS without client auth.
    pub fn generate() -> Result<Self, io::Error> {
        let env_relaxed = std::env::var("TAGLIACARTE_MAIL_BODY_TLS_NO_CLIENT_CERT")
            .ok()
            .as_deref()
            == Some("1");
        let want_mtls = mail_body_tls_requires_client_cert() && !env_relaxed;
        if want_mtls {
            Self::generate_with_mtls()
        } else {
            Self::generate_tls_server_only()
        }
    }

    /// Ephemeral TLS with mutual authentication (WebView must present [`Self::client_cert_pem`]).
    pub fn generate_with_mtls() -> Result<Self, io::Error> {
        let ca_key = KeyPair::generate().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut ca_dn = DistinguishedName::new();
        ca_dn.push(DnType::CommonName, "tagliacarte-mail-body-ca");
        let mut ca_params =
            CertificateParams::new(vec![]).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        ca_params.distinguished_name = ca_dn;
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let ca_cert = ca_params
            .self_signed(&ca_key)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let server_key =
            KeyPair::generate().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut server_params = CertificateParams::new(vec!["localhost".to_string()])
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        server_params
            .subject_alt_names
            .push(SanType::IpAddress(std::net::IpAddr::V4(
                std::net::Ipv4Addr::LOCALHOST,
            )));
        let server_cert = server_params
            .signed_by(&server_key, &ca_cert, &ca_key)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let client_key =
            KeyPair::generate().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut client_dn = DistinguishedName::new();
        client_dn.push(DnType::CommonName, "tagliacarte-mail-body-client");
        let mut client_params =
            CertificateParams::new(vec![]).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        client_params.distinguished_name = client_dn;
        let client_cert = client_params
            .signed_by(&client_key, &ca_cert, &ca_key)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let ca_cert_pem = ca_cert.pem();
        let client_cert_pem = client_cert.pem();
        let client_key_pem = client_key.serialize_pem();

        let creds = server_credentials_from_pem_bytes(
            server_cert.pem().as_bytes(),
            server_key.serialize_pem().as_bytes(),
        )?;
        let tls_acceptor = acceptor_from_credentials_with_client_auth(
            creds,
            ALPN,
            ClientAuthPolicy::Require,
            &[Bytes::from(ca_cert.der().to_vec())],
        );

        Ok(Self {
            ca_cert_pem,
            client_cert_pem,
            client_key_pem,
            tls_acceptor,
            enforces_client_cert: true,
        })
    }

    /// TLS to loopback with server cert only (no client certificate required).
    fn generate_tls_server_only() -> Result<Self, io::Error> {
        let ca_key = KeyPair::generate().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut ca_dn = DistinguishedName::new();
        ca_dn.push(DnType::CommonName, "tagliacarte-mail-body-ca");
        let mut ca_params =
            CertificateParams::new(vec![]).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        ca_params.distinguished_name = ca_dn;
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let ca_cert = ca_params
            .self_signed(&ca_key)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let server_key =
            KeyPair::generate().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut server_params = CertificateParams::new(vec!["localhost".to_string()])
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        server_params
            .subject_alt_names
            .push(SanType::IpAddress(std::net::IpAddr::V4(
                std::net::Ipv4Addr::LOCALHOST,
            )));
        let server_cert = server_params
            .signed_by(&server_key, &ca_cert, &ca_key)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let client_key =
            KeyPair::generate().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut client_dn = DistinguishedName::new();
        client_dn.push(DnType::CommonName, "tagliacarte-mail-body-client");
        let mut client_params =
            CertificateParams::new(vec![]).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        client_params.distinguished_name = client_dn;
        let client_cert = client_params
            .signed_by(&client_key, &ca_cert, &ca_key)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let ca_cert_pem = ca_cert.pem();
        let client_cert_pem = client_cert.pem();
        let client_key_pem = client_key.serialize_pem();

        let creds = server_credentials_from_pem_bytes(
            server_cert.pem().as_bytes(),
            server_key.serialize_pem().as_bytes(),
        )?;
        let tls_acceptor = acceptor_from_credentials(creds, ALPN);

        Ok(Self {
            ca_cert_pem,
            client_cert_pem,
            client_key_pem,
            tls_acceptor,
            enforces_client_cert: false,
        })
    }
}
