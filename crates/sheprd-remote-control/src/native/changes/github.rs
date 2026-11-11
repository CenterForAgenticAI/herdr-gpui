//! Host-local, explicitly enabled GitHub reads. No desktop credential is
//! imported, persisted, or returned. One blocking reader bounds concurrency
//! across background badges and all paired phones; cache misses are paced.

use crate::control::changes::{Answer, Check, CheckLog, Checks, Failure, MAX_CHECKS, Wanted};
use sheprd_github::{
    check_log,
    pr_actions::Target,
    pull_request::{self, Input, Outcome, PullRequest},
};
use std::{
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};

const TTL: Duration = Duration::from_secs(60);
const PACE: Duration = Duration::from_secs(2);
const LIMIT: usize = 128;

#[derive(Default)]
pub(crate) struct Reader {
    enabled: bool,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    entries: Vec<Entry>,
    next: Option<Instant>,
}

struct Entry {
    input: Input,
    head: String,
    repository: String,
    until: Instant,
    value: Result<Option<PullRequest>, Failure>,
    logs: Vec<(u64, Result<CheckLog, Failure>)>,
}

impl Reader {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            cache: Mutex::default(),
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// Blocking; callers must use the blocking pool. Busy requests are
    /// refused rather than accumulating threads behind a slow GitHub read.
    pub(crate) fn answer(&self, cwd: &str, wanted: &Wanted) -> Result<Answer, Failure> {
        self.answer_using(
            cwd,
            wanted,
            |input| {
                sheprd_github::cli::token()
                    .and_then(|token| pull_request::fetch(input, &token, || false))
            },
            |target, job| {
                sheprd_github::cli::token()
                    .and_then(|token| check_log::job_log(&token, &target.owner, &target.repo, job))
            },
        )
    }

    fn answer_using(
        &self,
        cwd: &str,
        wanted: &Wanted,
        load: impl FnOnce(&Input) -> sheprd_github::Result<Option<PullRequest>>,
        log: impl FnOnce(&Target, u64) -> sheprd_github::Result<String>,
    ) -> Result<Answer, Failure> {
        self.with_entry_using(cwd, load, |cache, index| {
            let entry = &cache.entries[index];
            let pr = entry.value.as_ref().map_err(|error| *error)?;
            match wanted {
                Wanted::Checks => Ok(Answer::Checks(checks(pr.as_ref()))),
                Wanted::CheckLog { job } => {
                    let pr = pr.as_ref().ok_or(Failure::NoLog)?;
                    if !checks(Some(pr))
                        .checks
                        .iter()
                        .any(|check| check.job == Some(*job))
                    {
                        return Err(Failure::NoLog);
                    }
                    if let Some((_, log)) = entry.logs.iter().find(|(id, _)| id == job) {
                        return log.clone().map(Answer::CheckLog);
                    }
                    let target = Target::for_reading(pr).map_err(failure)?;
                    cache.reserve(Instant::now())?;
                    let result = log(&target, *job);
                    let log = match result {
                        Ok(text) => {
                            let excerpt = check_log::excerpt(&text);
                            if excerpt.is_empty() {
                                Err(Failure::NoLog)
                            } else {
                                Ok(CheckLog { excerpt })
                            }
                        }
                        Err(error) => {
                            cache.pause(&error);
                            Err(failure(error))
                        }
                    };
                    cache.entries[index].logs.push((*job, log.clone()));
                    log.map(Answer::CheckLog)
                }
                _ => Err(Failure::Unreadable),
            }
        })
    }

    pub(crate) fn pull_request(
        &self,
        cwd: &str,
        branch: &str,
    ) -> Result<Option<PullRequest>, Failure> {
        self.with_entry(cwd, |cache, index| {
            let entry = &cache.entries[index];
            if entry.input.branch != branch {
                return Err(Failure::Gone);
            }
            entry.value.clone()
        })
    }

    fn with_entry<T>(
        &self,
        cwd: &str,
        read: impl FnOnce(&mut Cache, usize) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        self.with_entry_using(
            cwd,
            |input| {
                sheprd_github::cli::token()
                    .and_then(|token| pull_request::fetch(input, &token, || false))
            },
            read,
        )
    }

    fn with_entry_using<T>(
        &self,
        cwd: &str,
        load: impl FnOnce(&Input) -> sheprd_github::Result<Option<PullRequest>>,
        read: impl FnOnce(&mut Cache, usize) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        if !self.enabled {
            return Err(Failure::GitHubSignIn);
        }
        if cwd.is_empty() || cwd.len() > 4096 {
            return Err(Failure::NoRepository);
        }
        let mut cache = self.cache.try_lock().map_err(|_| Failure::RateLimited)?;
        let cwd = Path::new(cwd);
        let branch = super::local::git(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .map_err(|_| Failure::NoRepository)?
            .trim()
            .to_owned();
        let head = super::local::git(cwd, &["rev-parse", "--verify", "HEAD"])
            .map_err(|_| Failure::NoRepository)?
            .trim()
            .to_owned();
        let repository = super::local::git(
            cwd,
            &[
                "config",
                "--local",
                "--get-regexp",
                "^(remote\\.(origin|upstream)\\.url|branch\\..*\\.(remote|merge))$",
            ],
        )
        .unwrap_or_default();
        if branch.len() > 512 || repository.len() > 8192 {
            return Err(Failure::NoRepository);
        }
        let input = Input {
            checkout: Some(cwd.to_string_lossy().into_owned()),
            repo_key: None,
            branch,
        };
        let now = Instant::now();
        let index = match cache.entries.iter().position(|e| {
            e.input == input && e.head == head && e.repository == repository && now < e.until
        }) {
            Some(index) => index,
            None => {
                cache.reserve(now)?;
                let result = load(&input);
                let value = result.map_err(|error| {
                    cache.pause(&error);
                    failure(error)
                });
                let current_branch =
                    super::local::git(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"])
                        .map_err(|_| Failure::Gone)?;
                let current_head = super::local::git(cwd, &["rev-parse", "--verify", "HEAD"])
                    .map_err(|_| Failure::Gone)?;
                if current_branch.trim() != input.branch || current_head.trim() != head {
                    return Err(Failure::Gone);
                }
                cache
                    .entries
                    .retain(|entry| entry.input.checkout != input.checkout);
                if cache.entries.len() == LIMIT {
                    cache.entries.remove(0);
                }
                cache.entries.push(Entry {
                    input,
                    head,
                    repository,
                    until: Instant::now() + TTL,
                    value,
                    logs: Vec::new(),
                });
                cache.entries.len() - 1
            }
        };
        read(&mut cache, index)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

impl Cache {
    fn reserve(&mut self, now: Instant) -> Result<(), Failure> {
        if self.next.is_some_and(|next| now < next) {
            return Err(Failure::RateLimited);
        }
        self.next = Some(now + PACE);
        Ok(())
    }

    fn pause(&mut self, error: &sheprd_github::Error) {
        // Account-wide, including requests for other repositories and logs.
        let delay = match error {
            sheprd_github::Error::RateLimit { .. } => Duration::from_secs(60 * 60),
            _ => TTL,
        };
        self.next = Some(Instant::now() + delay);
    }
}

fn checks(pr: Option<&PullRequest>) -> Checks {
    let Some(pr) = pr else {
        return Checks::default();
    };
    Checks {
        number: Some(pr.number),
        checks: pr
            .check_details()
            .take(MAX_CHECKS)
            .map(|detail| Check {
                name: detail.name.to_owned(),
                state: detail.outcome.into(),
                job: detail.job.filter(|_| detail.outcome == Outcome::Failed),
                url: detail.url.map(str::to_owned),
            })
            .collect(),
    }
}

fn failure(error: sheprd_github::Error) -> Failure {
    use sheprd_github::Error;
    match error {
        Error::Authentication | Error::Cli(_) | Error::Token => Failure::GitHubSignIn,
        Error::Forbidden | Error::RepositoryHidden { .. } => Failure::GitHubScope,
        Error::RateLimit { .. } => Failure::RateLimited,
        Error::Network(_) | Error::Read(_) | Error::Status(500..=599) => Failure::GitHubUnavailable,
        Error::Status(404 | 410) => Failure::NoLog,
        Error::CheckLog(check_log::Error::TooLong) => Failure::LogTooLong,
        _ => Failure::Unreadable,
    }
}

/// Incremental prefetch: at most one checkout per tick, shared with on-demand
/// reads. Publish only against the current boot, folder and branch identity.
pub(crate) async fn badges(state: crate::web::Shared) {
    use crate::control::pull_requests::{MAX_PULL_REQUESTS, PullRequests, WorkspacePullRequest};
    use std::sync::Arc;
    let mut cursor = 0_usize;
    let mut tick = tokio::time::interval(PACE);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        let layout = state.hub.layout();
        let workspaces: Vec<_> = layout
            .workspaces
            .iter()
            .filter(|workspace| !workspace.cwd.is_empty() && workspace.branch.is_some())
            .take(MAX_PULL_REQUESTS)
            .collect();
        if workspaces.is_empty() {
            continue;
        }
        cursor %= workspaces.len();
        let workspace = workspaces[cursor].clone();
        cursor += 1;
        let worker = state.clone();
        let cwd = workspace.cwd.clone();
        let branch = workspace.branch.clone().unwrap_or_default();
        let result =
            tokio::task::spawn_blocking(move || worker.github.pull_request(&cwd, &branch)).await;
        let current = state.hub.layout();
        if current.boot != layout.boot
            || current
                .workspace(&workspace.id)
                .is_none_or(|now| now.cwd != workspace.cwd || now.branch != workspace.branch)
        {
            continue;
        }
        let previous = state.pull_requests.borrow().clone();
        let mut snapshot = if previous.boot_id == current.boot {
            (*previous).clone()
        } else {
            PullRequests {
                boot_id: current.boot.clone(),
                ..Default::default()
            }
        };
        snapshot.entries.retain(|entry| {
            entry.workspace_id != workspace.id
                && current.workspace(&entry.workspace_id).is_some_and(|now| {
                    now.cwd == entry.cwd && now.branch.as_deref() == Some(&entry.branch)
                })
        });
        if let Ok(Ok(Some(pr))) = result {
            let entry = WorkspacePullRequest {
                workspace_id: workspace.id,
                cwd: workspace.cwd,
                branch: workspace.branch.unwrap_or_default(),
                number: pr.number,
                tone: pr.tone(),
                url: pr.url,
            };
            if entry.valid() {
                snapshot.entries.push(entry);
            }
        }
        if snapshot.valid() && snapshot != *previous {
            state.pull_requests.send_replace(Arc::new(snapshot));
        }
    }
}
