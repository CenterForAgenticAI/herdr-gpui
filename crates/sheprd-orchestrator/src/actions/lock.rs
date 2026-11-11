//! Serialize mutations across windows/processes. The OS releases the lease
//! after a crash, unlike a persistent busy bit in the run database.
use std::{
    fs::File,
    path::PathBuf,
    time::{Duration, Instant},
};

/// How long a lease is retried before another holder is reported busy. A
/// child forked while a lease is held keeps the open file, and with it the
/// `flock`, until it execs, so a lease just dropped can still look held for
/// a moment. Actions run on their own thread, never the UI's.
const SETTLE: Duration = Duration::from_millis(1_500);
const RETRY: Duration = Duration::from_millis(10);

#[derive(Debug, thiserror::Error)]
#[error("Cannot lock orchestrator actions at {path}: {source}")]
pub struct Error {
    path: PathBuf,
    #[source]
    source: std::io::Error,
}

pub(super) fn take(site: &super::Site) -> crate::Result<File> {
    let path = site.database.with_extension("actions.lock");
    crate::private_file::prepare(&path)?;
    let file = File::options()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|source| Error {
            path: path.clone(),
            source,
        })?;
    let deadline = Instant::now() + SETTLE;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(RETRY);
            }
            Err(std::fs::TryLockError::WouldBlock) => return Err(crate::Error::Busy),
            Err(std::fs::TryLockError::Error(source)) => {
                return Err(Error { path, source }.into());
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
