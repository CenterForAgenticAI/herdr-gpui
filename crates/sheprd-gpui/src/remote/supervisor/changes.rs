//! The supervisor's part in answering phones about workspaces: while a phone
//! is connected it waits on the daemon for their questions, one long request
//! at a time, the next tick starting the next, and answers each on the
//! background executor. With no phone connected it asks nothing.
//!
//! The daemon's limit counts only questions a phone still waits for, and an
//! answer keeps running after its phone gives up, so the answers running here
//! are limited too. While that many run, or questions taken wait their turn,
//! no more are taken: the rest stay with the daemon, where a phone that stops
//! waiting frees its place.

use super::*;
use crate::remote::changes::Sources;
use sheprd_remote_control::control::changes::Ask;
use std::collections::VecDeque;

/// How long after a failed wait the daemon is asked again.
const RETRY: Duration = Duration::from_secs(5);

/// How many answers are worked out at once.
pub(super) const RUNNING: usize = 4;

#[derive(Default)]
pub(crate) struct Changes {
    polling: bool,
    /// Questions taken from the daemon that wait for a running answer to end.
    pub(super) queued: VecDeque<Ask>,
    retry_at: Option<Instant>,
    /// Answers being worked out now, at most `RUNNING`.
    pub(super) running: usize,
    sources: Arc<Sources>,
}

impl Supervisor {
    /// The workspaces phones may ask about, as a window last saw them.
    pub(super) fn set_change_sources(&mut self, sources: Sources) {
        self.changes.sources = Arc::new(sources);
    }

    /// Waits for phones' questions, unless that is underway or no phone
    /// could be asking.
    pub(super) fn poll_changes(cx: &mut App) {
        let executor = cx.background_executor().clone();
        let supervisor = cx.global_mut::<Self>();
        let phones = matches!(&supervisor.health, Health::Running(status) if status.connected > 0);
        let waiting = supervisor
            .changes
            .retry_at
            .is_some_and(|at| Instant::now() < at);
        let busy = supervisor.changes.running >= RUNNING || !supervisor.changes.queued.is_empty();
        if !phones || waiting || busy || supervisor.changes.polling {
            return;
        }
        let Some(address) = supervisor.address.clone() else {
            return;
        };
        supervisor.changes.polling = true;
        let send = supervisor.send;
        let task = executor.spawn(async move { send(&address, &Request::ChangeAsks) });
        cx.spawn(async move |cx| {
            let answer = task.await;
            cx.update(|cx| Self::take_asks(cx, answer));
        })
        .detach();
    }

    pub(super) fn take_asks(cx: &mut App, answer: Result<Response, control::Error>) {
        let supervisor = cx.global_mut::<Self>();
        supervisor.changes.polling = false;
        let Ok(Response::ChangeAsks { asks }) = answer else {
            // A daemon that does not know the request answers at once.
            supervisor.changes.retry_at = Some(Instant::now() + RETRY);
            return;
        };
        supervisor.changes.retry_at = None;
        supervisor.changes.queued.extend(asks);
        Self::start_answers(cx);
    }

    /// Starts queued answers while fewer than `RUNNING` run.
    fn start_answers(cx: &mut App) {
        let executor = cx.background_executor().clone();
        let supervisor = cx.global_mut::<Self>();
        let Some(address) = supervisor.address.clone() else {
            // With no daemon to answer, nobody waits for these.
            supervisor.changes.queued.clear();
            return;
        };
        let (send, sources) = (supervisor.send, supervisor.changes.sources.clone());
        let mut started = Vec::new();
        while supervisor.changes.running < RUNNING {
            let Some(ask) = supervisor.changes.queued.pop_front() else {
                break;
            };
            supervisor.changes.running += 1;
            let (address, sources) = (address.clone(), sources.clone());
            started.push(executor.spawn(async move { reply(&address, send, &sources, &ask) }));
        }
        for task in started {
            cx.spawn(async move |cx| {
                task.await;
                cx.update(|cx| {
                    let changes = &mut cx.global_mut::<Self>().changes;
                    changes.running = changes.running.saturating_sub(1);
                    Self::start_answers(cx);
                });
            })
            .detach();
        }
    }
}

/// Works out one answer and hands it to the daemon. Blocking.
fn reply(address: &Path, send: Send, sources: &Sources, ask: &Ask) {
    let answer = sources.answer(ask);
    let id = ask.id;
    if let Err(error) = send(address, &Request::ChangeAnswer { id, answer }) {
        tracing::debug!(%error, "The daemon did not take an answer for a phone");
    }
}
