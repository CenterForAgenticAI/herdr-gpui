#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("link_handlers: {0}")]
    Invalid(&'static str),
    #[error("link_handlers pattern: {0}")]
    Pattern(#[from] regex::Error),
    #[error("The clicked terminal's host is no longer available")]
    HostChanged,
    #[error("Link handler failed: {0}")]
    Run(#[from] sheprd_client::Error),
    #[cfg(feature = "cloud")]
    #[error("Link handler failed: {0}")]
    Cloud(#[from] std::io::Error),
}
