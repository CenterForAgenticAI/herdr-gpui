//! Claiming a remote work request. The claim is its own label beside the
//! request rather than a rewrite of it, so a caller's cancellation, which
//! removes the request, can never be undone by a claim written after it.

use sheprd_backend::TerminalId;
use sheprd_core_client::CoreBackend;
use std::collections::BTreeMap;

/// The value a claim marker holds; its key names the request it claims.
const CLAIMED: &str = "1";

pub(super) fn start(
    backend: CoreBackend,
    terminal: TerminalId,
    key: String,
    claim: String,
    expected: String,
) -> Option<std::sync::mpsc::Receiver<sheprd_core_client::Result<bool>>> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let spawned = std::thread::Builder::new()
        .name("remote-work-claim".into())
        .spawn(move || {
            let result = (|| {
                let listed = backend.core().list(false)?;
                let Some(labels) = listed
                    .terminals
                    .iter()
                    .find(|info| info.address == terminal.as_str())
                    .map(|info| &info.labels)
                else {
                    return Ok(false);
                };
                if !open(labels, &key, &claim, &expected) {
                    return Ok(false);
                }
                // The core applies label writes in order and answers with the
                // labels after this one. A request already withdrawn is gone
                // from that answer; one withdrawn later finds the marker.
                let marked = backend.core().label(
                    terminal.as_str(),
                    &BTreeMap::from([(claim.as_str(), CLAIMED)]),
                )?;
                if held(&marked.labels, &key, &claim, &expected) {
                    return Ok(true);
                }
                backend
                    .core()
                    .label(terminal.as_str(), &BTreeMap::from([(claim.as_str(), "")]))?;
                Ok(false)
            })();
            let _ = send.send(result);
        });
    match spawned {
        Ok(_) => Some(receive),
        Err(error) => {
            tracing::warn!(%error, "Could not start remote work claim");
            None
        }
    }
}

/// The request still stands as it was read and nothing has claimed it.
fn open(labels: &BTreeMap<String, String>, key: &str, claim: &str, expected: &str) -> bool {
    labels.get(key).is_some_and(|value| value == expected) && !labels.contains_key(claim)
}

/// After marking: the request still stands and the marker is this claim's.
fn held(labels: &BTreeMap<String, String>, key: &str, claim: &str, expected: &str) -> bool {
    labels.get(key).is_some_and(|value| value == expected)
        && labels.get(claim).is_some_and(|value| value == CLAIMED)
}

#[cfg(test)]
mod tests;
