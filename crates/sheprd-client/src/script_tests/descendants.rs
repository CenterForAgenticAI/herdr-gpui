use super::*;

#[test]
fn an_exited_parent_does_not_disable_pipe_deadlines() {
    #[cfg(unix)]
    let command = {
        let mut command = crate::new_command("/bin/sh");
        command.args(["-c", "sleep 30 &"]);
        command
    };
    #[cfg(windows)]
    let command = {
        let mut command = crate::new_command("cmd.exe");
        command.args([
            "/D",
            "/S",
            "/C",
            "start /B cmd.exe /D /S /C \"ping -n 31 127.0.0.1 >nul\"",
        ]);
        command
    };
    let start = std::time::Instant::now();
    let result = run_script_command(
        command,
        std::io::empty(),
        std::io::sink(),
        ScriptLimits {
            output: 1024,
            idle: Duration::from_millis(100),
        },
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(Error::ScriptTimeout)), "{result:?}");
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "pipe readers outlived their deadline"
    );
}
