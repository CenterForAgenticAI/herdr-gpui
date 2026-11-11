use std::{io, net::SocketAddr};

/// Why the QUIC transport could not start.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("could not listen for QUIC on {address}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("could not make the relay's certificate")]
    Certificate(#[from] rcgen::Error),
    #[error("the TLS setup has no cipher suite QUIC can start with")]
    Cipher(#[from] quinn::crypto::rustls::NoInitialCipherSuite),
    #[error("could not set up TLS")]
    Tls(#[from] rustls::Error),
    // keep-sorted end
}
