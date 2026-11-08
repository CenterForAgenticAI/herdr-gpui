use super::terminal;

#[test]
fn a_core_chat_key_names_its_terminal() {
    assert_eq!(terminal("core:t7").unwrap().as_str(), "t7");
    assert_eq!(terminal("core:box/t7").unwrap().as_str(), "box/t7");
    assert!(terminal("p_1").is_none());
    assert!(terminal("core:").is_none());
}
