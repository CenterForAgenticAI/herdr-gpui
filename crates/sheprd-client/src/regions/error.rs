//! Why a desktop-region command was not sent.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("this connection did not negotiate desktop regions")]
    NotNegotiated,
    // keep-sorted end
}
