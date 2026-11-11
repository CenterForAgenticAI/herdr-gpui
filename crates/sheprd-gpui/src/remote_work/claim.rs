use sheprd_backend::TerminalId;
use sheprd_core_client::CoreBackend;
use std::collections::BTreeMap;

pub(super) fn start(
    backend: CoreBackend,
    terminal: TerminalId,
    key: String,
    expected: String,
) -> Option<std::sync::mpsc::Receiver<sheprd_core_client::Result<bool>>> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let spawned = std::thread::Builder::new()
        .name("remote-work-claim".into())
        .spawn(move || {
            let result = (|| {
                let listed = backend.core().list(false)?;
                let present = listed
                    .terminals
                    .iter()
                    .find(|info| info.address == terminal.as_str())
                    .and_then(|info| info.labels.get(&key))
                    .is_some_and(|value| value == &expected);
                if !present {
                    return Ok(false);
                }
                backend.core().label(
                    terminal.as_str(),
                    &BTreeMap::from([(key.as_str(), "claimed")]),
                )?;
                Ok(true)
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
