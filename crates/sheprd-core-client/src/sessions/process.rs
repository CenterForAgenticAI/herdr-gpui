use super::Error;
use crate::Result;
use sheprd_client::{ScriptHost, ScriptLimits, run_script};
use std::{sync::atomic::AtomicBool, time::Duration};

pub(super) struct Process {
    pub(super) parent: u32,
    pub(super) started: String,
    name: String,
}

pub(super) fn read(pid: u32) -> Result<Process> {
    if pid == 0 {
        return Err(Error::Identity.into());
    }
    // Linux supplies exact start ticks. ps on macOS supplies its UTC birth
    // second (as Claude's procStart records do); this is not an atomic handle.
    let script = format!(
        r#"set -f
case "$(uname -s)" in
Linux)
 IFS= read -r stat < /proc/{pid}/stat || exit 1
 fields=${{stat##*) }}
 set -- $fields
 [ "$#" -ge 20 ] || exit 1
 comm=${{stat#*(}}
 comm=${{comm%)*}}
 printf '%s\nlinux:%s\n%s\n' "$2" "${{20}}" "$comm"
 ;;
Darwin)
 LC_ALL=C ps -o ppid= -p {pid} || exit 1
 LC_ALL=C TZ=UTC ps -o lstart= -p {pid} || exit 1
 LC_ALL=C ps -o comm= -p {pid} || exit 1
 ;;
*) exit 1;;
esac
"#
    );
    let mut output = Vec::new();
    run_script(
        ScriptHost::Local,
        &script,
        &[][..],
        &mut output,
        ScriptLimits {
            output: 1024,
            idle: Duration::from_secs(2),
        },
        &AtomicBool::new(false),
    )
    .map_err(Error::Probe)?;
    let text = std::str::from_utf8(&output).map_err(|_| Error::Identity)?;
    let mut lines = text.lines();
    let parent = lines
        .next()
        .and_then(|word| word.trim().parse().ok())
        .ok_or(Error::Identity)?;
    let started = lines
        .next()
        .ok_or(Error::Identity)?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let name = lines.next().ok_or(Error::Identity)?.trim().to_owned();
    if started.is_empty() || name.is_empty() || lines.next().is_some() {
        return Err(Error::Identity.into());
    }
    Ok(Process {
        parent,
        started,
        name,
    })
}

pub(super) fn descends(mut pid: u32, ancestor: u32) -> Result<bool> {
    for depth in 0..32 {
        if pid == ancestor {
            return Ok(true);
        }
        let process = read(pid)?;
        // Only shell hook launchers may intervene. A nested agent inherits
        // HC_TERMINAL_ID too; ancestry alone would misattribute its session.
        if depth > 0
            && !matches!(
                process
                    .name
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('-'),
                "sh" | "bash" | "zsh" | "dash" | "fish" | "ksh"
            )
        {
            return Ok(false);
        }
        let parent = process.parent;
        if parent == 0 || parent == pid {
            return Ok(false);
        }
        pid = parent;
    }
    Ok(false)
}
