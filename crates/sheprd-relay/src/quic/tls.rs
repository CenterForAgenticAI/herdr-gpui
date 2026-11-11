//! The relay's TLS identity for QUIC, and how clients trust it.
//!
//! The relay makes a fresh P-256 key and self-signed certificate at every
//! start, so it needs no volume, no ACME, and no file shared with the
//! HTTPS proxy. Clients pin the certificate's SHA-256, which they read from
//! `GET /v1/info` over the relay's ordinary HTTPS (or a cached copy), and
//! read it again when a handshake fails after a restart.

use super::Error;
use crate::wire::ALPN;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use std::{fmt, sync::Arc};

/// The name in the relay's certificate. Clients check the pin, not names.
pub const SERVER_NAME: &str = "sheprd-relay";

/// A certificate's SHA-256, as clients pin it.
pub type Pin = [u8; 32];

/// The relay's certificate and key for this run.
pub struct Identity {
    certificate: CertificateDer<'static>,
    key: PrivatePkcs8KeyDer<'static>,
    pub pin: Pin,
}

impl fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Identity")
            .field("pin", &self.pin)
            .finish_non_exhaustive()
    }
}

impl Identity {
    /// A fresh key and self-signed certificate.
    pub fn generate() -> Result<Self, Error> {
        let certified = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_owned()])?;
        let certificate = certified.cert.der().clone();
        let pin = Sha256::digest(&certificate).into();
        let key = PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der());
        Ok(Self {
            certificate,
            key,
            pin,
        })
    }

    /// The rustls side of the QUIC server: TLS 1.3, ALPN `sheprd-relay/1`.
    pub fn server_crypto(&self) -> Result<QuicServerConfig, Error> {
        let mut config = rustls::ServerConfig::builder_with_provider(provider())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_no_client_auth()
            .with_single_cert(
                vec![self.certificate.clone()],
                PrivateKeyDer::Pkcs8(self.key.clone_key()),
            )?;
        config.alpn_protocols = vec![ALPN.to_vec()];
        Ok(QuicServerConfig::try_from(config)?)
    }
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// A client config that trusts only the certificate with `pin`.
pub fn client_crypto(pin: Pin) -> Result<QuicClientConfig, Error> {
    let provider = provider();
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned { pin, provider }))
        .with_no_client_auth();
    config.alpn_protocols = vec![ALPN.to_vec()];
    Ok(QuicClientConfig::try_from(config)?)
}

/// Accepts exactly the pinned certificate, and still checks that the
/// handshake was signed by its key.
#[derive(Debug)]
struct Pinned {
    pin: Pin,
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let seen: Pin = Sha256::digest(end_entity).into();
        if seen == self.pin {
            return Ok(ServerCertVerified::assertion());
        }
        Err(rustls::Error::InvalidCertificate(
            rustls::CertificateError::ApplicationVerificationFailure,
        ))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            certificate,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            certificate,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
