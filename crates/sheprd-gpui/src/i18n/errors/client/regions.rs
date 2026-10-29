//! `sheprd_client::regions::Error`: a desktop-region command the client did
//! not send, from `error-client-regions.ftl`.

use super::Localize;
use crate::i18n::fl;
use sheprd_client::regions::Error;

impl Localize for Error {
    fn localize(&self) -> String {
        match self {
            // keep-sorted start
            Self::NotNegotiated => fl!("error-client-regions-not-negotiated"),
            // keep-sorted end
        }
    }
}
