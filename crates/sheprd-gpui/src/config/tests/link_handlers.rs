use super::*;

#[test]
fn link_handler_configuration_is_checked_and_compiled() {
    let text = r#"
[[link_handlers]]
plugin = "fixture"
id = "notes"
host = "local"
pattern = '^file://.*\.md$'
directory = "/trusted/plugin"
command = ["./open-link"]
"#;
    let config = Config::parse_over(text, &Daemon::default()).unwrap();
    assert!(config.unknown_keys.is_empty());
    assert!(
        config
            .link_handlers
            .matching("local", "file:///notes.md")
            .is_some()
    );
    assert!(
        config
            .link_handlers
            .matching("ssh:other", "file:///notes.md")
            .is_none()
    );
    assert!(
        Config::parse_over(&text.replace("^file://.*\\.md$", "["), &Daemon::default()).is_err()
    );
}
