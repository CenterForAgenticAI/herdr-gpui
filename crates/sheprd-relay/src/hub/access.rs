//! Who may listen on a relay: anyone, by default, so a relay needs no
//! accounts and no setup; or, optionally, holders of one token, for a relay
//! kept to its owner's computers. Who may *dial* a computer is the
//! computer's choice (its dial keys), never the operator's.

use crate::secret::Secret;

/// Who may listen.
#[derive(Debug, Default)]
pub enum Access {
    /// Anyone who proves a key.
    #[default]
    Open,
    /// Holders of the operator's one token.
    Token(Secret),
}

impl Access {
    /// Whether `token` may listen.
    #[must_use]
    pub fn admit(&self, token: Option<&[u8]>) -> bool {
        match (self, token) {
            (Self::Open, _) => true,
            (Self::Token(secret), Some(token)) => secret.matches(token),
            (Self::Token(_), None) => false,
        }
    }

    /// Whether listening needs a token.
    #[must_use]
    pub fn requires_token(&self) -> bool {
        matches!(self, Self::Token(_))
    }
}
