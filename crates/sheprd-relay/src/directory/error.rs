/// Why a relay list cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("relay {index} has an endpoint that is not a relay address")]
    Endpoint {
        index: usize,
        #[source]
        source: crate::endpoint::Error,
    },
    #[error("the relay list is not the expected JSON")]
    Format,
    #[error("relay {index} is not https")]
    Insecure { index: usize },
    #[error("a relay's {field} is too long or not printable")]
    Label { field: &'static str },
    #[error("the relay list is larger than 64 KiB")]
    TooLarge,
    #[error("the relay list names more than 64 relays")]
    TooMany,
    #[error("the relay list is version {version}; this client reads version 1")]
    Version { version: u32 },
    // keep-sorted end
}
