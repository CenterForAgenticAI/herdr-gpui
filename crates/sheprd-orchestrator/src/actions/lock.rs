//! Serialize mutations across windows/processes. The OS releases the lease
//! after a crash, unlike a persistent busy bit in the run database.
use std::{fs::File, path::PathBuf};

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
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(crate::Error::Busy),
        Err(std::fs::TryLockError::Error(source)) => Err(Error { path, source }.into()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
