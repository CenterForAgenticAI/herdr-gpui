use super::*;
use sheprd_chat_protocol::{Agent, ENDPOINT_ENV, TOKEN};
use std::path::Path;

#[test]
fn a_lost_chat_keeps_its_terminal_and_folder_like_a_desktop_chat() {
    let (source, fake) = fake();
    source.poll().unwrap();
    source
        .open_chat(Agent::Claude, Path::new("/bin/sheprd-acp"), "/code", None)
        .unwrap();
    let request = fake.inputs.recv_timeout(Duration::from_secs(2)).unwrap();
    let request: Value = serde_json::from_slice(&request).unwrap();
    assert_eq!(request["method"], "terminal.run");
    let params = &request["params"];
    assert_eq!(params["keep"], true);
    assert_eq!(params["cwd"], "/code");
    assert_eq!(params["env"]["HERDR_PANE_ID"], "");
    assert!(params["labels"][TOKEN].as_str().is_some());
    assert_eq!(params["labels"][TOKEN], params["env"][ENDPOINT_ENV]);

    // Ordinary shells still recover at the same address after program loss.
    source.open_shell("/code", None).unwrap();
    let request = fake.inputs.recv_timeout(Duration::from_secs(2)).unwrap();
    let request: Value = serde_json::from_slice(&request).unwrap();
    assert_eq!(request["params"]["keep"], true);
    drop(source);
    fake.server.join().unwrap();
}

#[test]
fn the_recovery_shell_is_a_terminal_not_a_chat_despite_its_saved_label() {
    let mut terminal = record("t9");
    terminal["labels"] = json!({TOKEN: "0123456789abcdef0123456789abcdef"});
    terminal["keep"] = json!(true);
    terminal["reason"] = json!("lost");
    terminal["pid"] = json!(42);
    terminal["foreground"] = json!({"pid": 42, "name": "zsh", "argv": ["zsh"], "cwd": "/code"});
    let (source, fake) = fake_with_doc(false, Value::Null, vec![terminal]);
    let layout = source.poll().unwrap();
    assert!(layout.chat(&id("t9")).is_none());
    assert!(layout.terminal(&id("t9")).is_some());
    drop(source);
    fake.server.join().unwrap();
}
