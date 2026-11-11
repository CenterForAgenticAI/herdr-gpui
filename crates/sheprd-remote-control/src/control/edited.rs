//! Settings a computer's own daemon keeps for every phone and desktop that
//! reaches it, such as its tags and the composer's text shortcuts. A client
//! offers its copy with the time it was edited and the daemon answers with
//! the copy that stands: the latest edit wins, whole.

use serde::{Serialize, de::DeserializeOwned};

/// A setting kept whole, with the time of its last edit.
pub trait Edited: Clone + Default + Eq + Serialize + DeserializeOwned {
    /// Milliseconds since the Unix epoch; 0 for one never edited.
    fn updated_ms(&self) -> u64;

    /// Itself within bounds, as edited at `updated_ms`. What a file or a
    /// peer holds is tidied through this before it is kept.
    #[must_use]
    fn bounded(self, updated_ms: u64) -> Self;

    /// What stands once `offer` meets this copy at `now_ms`. A clock running
    /// ahead cannot make an edit outlast every later one, so an offer counts
    /// as made by `now_ms` at the latest. On a tie the stored copy stays, so
    /// offering the same copy twice changes nothing.
    #[must_use]
    fn merged(self, offer: Self, now_ms: u64) -> Self {
        let updated_ms = offer.updated_ms().min(now_ms);
        if updated_ms > self.updated_ms() {
            offer.bounded(updated_ms)
        } else {
            self
        }
    }
}
