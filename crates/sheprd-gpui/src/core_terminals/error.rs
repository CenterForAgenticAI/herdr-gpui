#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("Could not answer from the terminal's history")]
    Answer,
    #[error(transparent)]
    Core(#[from] sheprd_core_client::Error),
    #[error("Could not save the terminal's history for the editor")]
    EditFile(#[source] std::io::Error),
    #[error("Could not read the terminal's history")]
    History(#[source] std::sync::Arc<dyn std::error::Error + Send + Sync>),
    #[error("The selected text moved before it was read")]
    Moved,
    #[error("This build has no herdr-core beside it; install herdr-core from releases.herdr.dev")]
    NotBundled,
    #[error("Could not start a worker for the terminal: {0}")]
    Worker(#[source] std::io::Error),
    // keep-sorted end
}
