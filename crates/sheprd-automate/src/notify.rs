//! A `notify` step on a host without Herdr 1, whose server would show it:
//! the system's own notification, from the daemon, so it reaches the user
//! while Sheprd is closed too. Title and body go as arguments, never as
//! script text, so neither can run as code.

use std::process::Stdio;

/// The most of a failed notifier's complaint kept.
const MAX_COMPLAINT_CHARS: usize = 300;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{program} could not show the notification: {complaint}")]
    Failed {
        program: &'static str,
        complaint: String,
    },
    #[error("could not run {program}")]
    Launch {
        program: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("notifications need Herdr 1 on this system")]
    Unsupported,
}

/// Shows `body` under `title` as a system notification.
pub fn show(title: &str, body: &str) -> Result<(), Error> {
    let (program, args) = command(title, body).ok_or(Error::Unsupported)?;
    let output = sheprd_client::new_command(program)
        .args(&args)
        .stdin(Stdio::null())
        .output()
        .map_err(|source| Error::Launch { program, source })?;
    if output.status.success() {
        return Ok(());
    }
    let complaint = String::from_utf8_lossy(&output.stderr);
    Err(Error::Failed {
        program,
        complaint: complaint.trim().chars().take(MAX_COMPLAINT_CHARS).collect(),
    })
}

/// The program and arguments that show a notification here.
fn command(title: &str, body: &str) -> Option<(&'static str, Vec<String>)> {
    if cfg!(target_os = "macos") {
        let script = "on run argv\ndisplay notification (item 2 of argv) with title (item 1 of argv)\nend run";
        return Some((
            "osascript",
            vec!["-e".into(), script.into(), title.into(), body.into()],
        ));
    }
    if cfg!(all(unix, not(target_os = "macos"))) {
        // `--` keeps a title starting with a dash from reading as an option.
        return Some((
            "notify-send",
            vec![
                "--app-name=Sheprd".into(),
                "--".into(),
                title.into(),
                body.into(),
            ],
        ));
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
