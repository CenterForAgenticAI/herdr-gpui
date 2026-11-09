//! What a finished move still asks of the app: closing the source's Herdr 2
//! tabs and opening the ones rebuilt on the destination. The window that
//! started it does this; when that window closed first, the move leaves it
//! here for whichever window ticks next, since the app, not a daemon, keeps
//! Herdr 2 tabs.

use super::{Error, Event, Outcome};
use crate::{
    SheprdWindow,
    browser::{Location, Scope, Store, TabId},
    i18n::fl,
    window::Flash,
};
use gpui::{Context, Global, Window};
use sheprd_session::panes::Panes;
use std::sync::{Arc, Mutex, PoisonError, mpsc};

/// A move that finished after its window closed.
pub(super) struct Leftover {
    pub(super) outcome: Outcome,
    /// Where the destination's tabs belong, read by the window that started
    /// the move: another window may name a different session `local`.
    pub(super) scope: Scope,
    /// The source's Herdr 2 tabs and the panes that moved from each.
    pub(super) closing: Vec<(TabId, Panes)>,
}

/// Moves that finished with no window to take them, shared with the
/// workers that run them.
#[derive(Clone, Default)]
pub(super) struct Unclaimed(Arc<Mutex<Vec<Leftover>>>);

impl Global for Unclaimed {}

impl Unclaimed {
    pub(super) fn push(&self, leftover: Leftover) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(leftover);
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    }

    fn take(&self) -> Vec<Leftover> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

/// Hands a move's `result` to its window, or, when that window closed, a
/// finished move's leftover work to `unclaimed`.
pub(super) fn deliver(
    sender: &mpsc::Sender<Event>,
    result: Result<Outcome, Error>,
    (scope, closing): (Scope, Vec<(TabId, Panes)>),
    unclaimed: &Unclaimed,
) {
    if let Err(mpsc::SendError(Event::Moved(Ok(outcome)))) = sender.send(Event::Moved(result)) {
        unclaimed.push(Leftover {
            outcome,
            scope,
            closing,
        });
    }
}

/// Of the source's Herdr 2 tabs, those whose every terminal moved. `now`
/// gives a tab's panes today. The window drops ended panes by itself, so a
/// tab with fewer panes still closes; one split during the move holds a
/// terminal that did not move and stays open with it.
pub(super) fn moved_tabs(
    closing: Vec<(TabId, Panes)>,
    now: impl Fn(TabId) -> Option<Panes>,
) -> Vec<TabId> {
    closing
        .into_iter()
        .filter(|(tab, moved)| {
            now(*tab).is_some_and(|panes| {
                panes
                    .terminals()
                    .into_iter()
                    .all(|terminal| moved.contains(terminal))
            })
        })
        .map(|(tab, _)| tab)
        .collect()
}

impl SheprdWindow {
    /// Finishes the moves whose window closed before they did.
    pub(super) fn claim_teleports(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(unclaimed) = cx.try_global::<Unclaimed>().cloned() else {
            return;
        };
        for leftover in unclaimed.take() {
            let store = cx.try_global::<Store>();
            let panes = |tab| match store?.get(tab)?.location.as_ref()? {
                Location::CoreTerminal { panes, .. } => Some(panes.clone()),
                _ => None,
            };
            for tab in moved_tabs(leftover.closing, panes) {
                self.close_browser_tab(tab, window, cx);
            }
            self.open_teleported_core_tabs(&leftover.outcome, &leftover.scope, cx);
        }
    }

    /// The scope of this window's endpoint `endpoint_id`, where its
    /// workspaces' tabs belong.
    pub(super) fn endpoint_scope(&self, endpoint_id: &str) -> Scope {
        self.endpoints
            .iter()
            .find(|endpoint| endpoint.id == endpoint_id)
            .map_or_else(|| Scope::endpoint(endpoint_id), crate::browser::scope)
    }

    /// Opens the tabs a move rebuilt in the destination's Herdr 2 in its
    /// new workspace. Their terminals already run there; the window joins
    /// them as it shows the workspace.
    pub(super) fn open_teleported_core_tabs(
        &mut self,
        outcome: &Outcome,
        scope: &Scope,
        cx: &mut Context<Self>,
    ) {
        if outcome.core_tabs.is_empty() {
            return;
        }
        // Checked before the move, so only tabs opened meanwhile can leave
        // one out; the rest still open.
        let missed = Store::update(cx, |store| {
            outcome
                .core_tabs
                .iter()
                .filter(|panes| {
                    let location = Location::core_terminal((*panes).clone(), false);
                    store
                        .open(scope.clone(), &outcome.workspace_id, Some(location), None)
                        .is_none()
                })
                .count()
        });
        if missed > 0 {
            self.show_flash(Flash::warning(fl!("core-terminals-too-many-tabs")), cx);
        }
        cx.notify();
    }
}
