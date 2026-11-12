//! Taking Sheprd's relay list only when it is genuine. Either of two
//! signers, both compiled in, vouches for the exact bytes served at
//! [`super::URL`]:
//!
//! - [`Signer::Maintainer`]: Fabien Penso's OpenPGP key
//!   ([`MAINTAINER_FINGERPRINT`], published on pen.so so anyone can check
//!   it), whose signing subkey lives on a hardware key. `relays.json.asc`
//!   is its detached, armored signature (`just relays-sign`).
//! - [`Signer::Actions`]: the site workflow's Ed25519 key, a secret of the
//!   protected `relays` environment. `relays.json.sig` is its raw 64-byte
//!   signature.
//!
//! Changing sheprd.org, its host, or its DNS is then not enough to redirect
//! clients: one would also need a signer's key. A signed list must carry a
//! `serial` and an `expires` time, and a client refuses one that expired,
//! or is older than the list it keeps, so an old signed list cannot be
//! replayed.

use super::{Directory, Error};
use ed25519_dalek::{Signature, VerifyingKey};
use pgp::{
    composed::{Deserializable, DetachedSignature, SignedPublicKey},
    types::KeyDetails,
};

/// Where the maintainer's signature is served, beside the list.
pub const OPENPGP_URL: &str = "https://sheprd.org/relays.json.asc";
/// Where the site workflow's signature is served, beside the list.
pub const ED25519_URL: &str = "https://sheprd.org/relays.json.sig";
/// The maintainer's OpenPGP certificate, as `gpg --export --armor
/// --export-options export-minimal` writes it.
pub const MAINTAINER_KEY: &str = include_str!("../../keys/maintainer.asc");
/// The maintainer's primary key fingerprint, as published on pen.so.
pub const MAINTAINER_FINGERPRINT: &str = "310320A8CC1C5BA86AD09040C0451BADF7649BBF";
/// The site workflow's Ed25519 public key (`RELAYS_SIGNING_KEY`).
pub const ACTIONS_KEY: [u8; 32] = [
    0x71, 0x0f, 0x88, 0xa3, 0x8d, 0xd9, 0x58, 0xcd, 0x22, 0x30, 0x43, 0x62, 0x45, 0xe4, 0x7b, 0x9c,
    0xa1, 0x33, 0x15, 0x1b, 0x56, 0x48, 0x6c, 0x14, 0x5a, 0x3f, 0xd8, 0xcd, 0x42, 0xc8, 0x28, 0x88,
];
/// The largest signature file a client reads.
pub const MAX_SIGNATURE: usize = 16 * 1024;

/// Who vouched for a list, for Settings and `sheprd remote status` to show.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signer {
    /// The maintainer's OpenPGP key.
    Maintainer,
    /// The site workflow's Ed25519 key.
    Actions,
}

impl Signer {
    /// "Validated by" what, in a sentence's words.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Maintainer => "Fabien Penso (OpenPGP …F764 9BBF)",
            Self::Actions => "GitHub Actions (sheprd-labs/sheprd)",
        }
    }
}

/// The signature files served beside the list, those that were found.
#[derive(Clone, Copy, Debug, Default)]
pub struct Signatures<'a> {
    pub openpgp: Option<&'a [u8]>,
    pub ed25519: Option<&'a [u8]>,
}

/// A list a signer vouched for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signed {
    pub directory: Directory,
    /// When the list stops being taken, in Unix seconds.
    pub expires: u64,
    pub serial: u64,
    pub signer: Signer,
}

impl Directory {
    /// Reads `bytes` as served at [`super::URL`], taking it only when a
    /// signer vouched for those exact bytes, it has not expired at `now`
    /// (Unix seconds), and it is no older than the list a client keeps,
    /// whose serial is `kept`.
    pub fn verify(
        bytes: &[u8],
        signatures: Signatures<'_>,
        now: u64,
        kept: Option<u64>,
    ) -> Result<Signed, Error> {
        let signer = signer(bytes, signatures, now).ok_or(Error::Unsigned)?;
        let directory = Self::parse(bytes)?;
        let (Some(serial), Some(expires)) = (directory.serial, directory.expires) else {
            return Err(Error::Unstamped);
        };
        if expires <= now {
            return Err(Error::Expired { expires });
        }
        if let Some(kept) = kept.filter(|kept| serial < *kept) {
            return Err(Error::Stale { serial, kept });
        }
        Ok(Signed {
            directory,
            expires,
            serial,
            signer,
        })
    }
}

/// Which signer vouched for `bytes`, the maintainer first.
fn signer(bytes: &[u8], signatures: Signatures<'_>, now: u64) -> Option<Signer> {
    let readable = |signature: &[u8]| signature.len() <= MAX_SIGNATURE;
    if signatures
        .openpgp
        .is_some_and(|armored| readable(armored) && maintainer_signed(bytes, armored, now))
    {
        return Some(Signer::Maintainer);
    }
    if signatures
        .ed25519
        .is_some_and(|raw| readable(raw) && actions_signed(bytes, raw))
    {
        return Some(Signer::Actions);
    }
    None
}

/// The maintainer's certificate, when it is the one with the published
/// fingerprint.
#[must_use]
pub fn maintainer() -> Option<SignedPublicKey> {
    let (key, _) = SignedPublicKey::from_string(MAINTAINER_KEY).ok()?;
    let fingerprint = key.primary_key.fingerprint().to_string();
    fingerprint
        .eq_ignore_ascii_case(MAINTAINER_FINGERPRINT)
        .then_some(key)
}

/// Whether `armored` is the maintainer's signature over `bytes`: made by a
/// subkey the primary key bound for signing, with its backward signature,
/// and not expired at `now`.
fn maintainer_signed(bytes: &[u8], armored: &[u8], now: u64) -> bool {
    maintainer().is_some_and(|key| signed_by(&key, bytes, armored, now))
}

/// Whether `armored` is `key`'s signature over `bytes`, as
/// [`maintainer_signed`] checks it.
pub(crate) fn signed_by(key: &SignedPublicKey, bytes: &[u8], armored: &[u8], now: u64) -> bool {
    let Ok(text) = std::str::from_utf8(armored) else {
        return false;
    };
    let Ok((signature, _)) = DetachedSignature::from_string(text) else {
        return false;
    };
    key.public_subkeys.iter().any(|subkey| {
        let created = u64::from(subkey.key.created_at().as_secs());
        let signing = subkey.signatures.iter().any(|binding| {
            // A lifetime of zero means none, as the standard has it.
            let expires = binding
                .key_expiration_time()
                .map(|lifetime| u64::from(lifetime.as_secs()))
                .filter(|lifetime| *lifetime > 0)
                .map(|lifetime| created.saturating_add(lifetime));
            binding.key_flags().sign() && expires.is_none_or(|expires| now < expires)
        });
        signing
            && subkey.verify_bindings(&key.primary_key).is_ok()
            && signature.verify(&subkey.key, bytes).is_ok()
    })
}

/// Whether `raw` is the site workflow's signature over `bytes`.
fn actions_signed(bytes: &[u8], raw: &[u8]) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(&ACTIONS_KEY) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(raw) else {
        return false;
    };
    key.verify_strict(bytes, &signature).is_ok()
}
