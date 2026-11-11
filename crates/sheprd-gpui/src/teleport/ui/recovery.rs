//! Recover committed destination tabs after process loss. Disk work always
//! runs on a worker; retries are bounded and never launch or stop a terminal.

use super::*;
use sheprd_remote::teleport::arrival::{self, Receipt};
use std::{
    path::PathBuf,
    sync::{Mutex, PoisonError},
};

const RETRY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Recovery {
    pending: Arc<Mutex<Vec<(PathBuf, Receipt)>>>,
    scanning: Arc<AtomicBool>,
    next: Option<Instant>,
}

impl gpui::Global for Recovery {}

fn directory() -> Option<PathBuf> {
    Store::path().and_then(|path| Some(path.parent()?.join("teleport-arrivals")))
}

/// Synchronous by design: this is called on the teleport worker at the commit
/// point, before it may retire the source. A failed durable write fails the move.
pub(super) fn persist(
    scope: &Scope,
    outcome: &Outcome,
) -> sheprd_remote::teleport::error::Result<()> {
    if outcome.core_tabs.is_empty() {
        return Ok(());
    }
    let directory = directory().ok_or(arrival::Error::Unavailable)?;
    arrival::save(&directory, &Receipt::new(scope.as_str(), outcome))?;
    Ok(())
}

impl Recovery {
    fn scan(&mut self) {
        // Headless tests must never read the user's persisted arrivals.
        if cfg!(test) {
            return;
        }
        let now = Instant::now();
        if self.scanning.load(Ordering::Acquire) || self.next.is_some_and(|next| now < next) {
            return;
        }
        let (Some(directory), Some(saved)) = (directory(), Store::path()) else {
            return;
        };
        self.next = Some(now + RETRY);
        self.scanning.store(true, Ordering::Release);
        let scanning = self.scanning.clone();
        let pending = self.pending.clone();
        let started = std::thread::Builder::new().name("teleport-arrivals".into()).spawn(move || {
            match arrival::load(&directory) {
                Ok(receipts) => {
                    let receipts = receipts.into_iter().filter(|(path, receipt)| {
                        match arrival::release(path, receipt, &saved) {
                            Ok(done) => !done,
                            Err(error) => { tracing::warn!(%error, "Could not release a saved Teleport arrival"); true }
                        }
                    }).collect();
                    *pending.lock().unwrap_or_else(PoisonError::into_inner) = receipts;
                }
                Err(error) => tracing::warn!(%error, "Could not read pending Teleport arrivals"),
            }
            scanning.store(false, Ordering::Release);
        });
        if let Err(error) = started {
            self.scanning.store(false, Ordering::Release);
            tracing::warn!(%error, "Could not start Teleport arrival recovery");
        }
    }
}

impl SheprdWindow {
    pub(super) fn recover_teleport_arrivals(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let recovery = cx.default_global::<Recovery>();
        recovery.scan();
        let pending = std::mem::take(
            &mut *recovery
                .pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        for (_, receipt) in pending {
            let scope = Scope::endpoint(receipt.scope());
            self.settle_teleport(&receipt.outcome(), &scope, window, cx);
        }
    }
}
