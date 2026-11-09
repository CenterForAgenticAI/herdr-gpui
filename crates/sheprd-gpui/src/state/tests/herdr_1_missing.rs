//! A host without Herdr 1: none installed here, or a saved host whose bridge
//! found none, until Herdr 1 answers there again.

use super::*;
use sheprd_client::SshFailure;

#[test]
fn a_saved_host_is_missing_herdr_1_until_it_answers_again() {
    let mut state = LiveState::default();
    assert!(!state.herdr_1_missing());
    state.apply(ClientEvent::Disconnected {
        reason: "Herdr is not installed on the host".into(),
        ssh: Some(SshFailure::HerdrMissing),
    });
    assert!(state.herdr_1_missing());
    // Another failure says nothing about Herdr being there.
    state.apply(ClientEvent::Disconnected {
        reason: "unreachable".into(),
        ssh: Some(SshFailure::Unreachable),
    });
    assert!(!state.herdr_1_missing());
    state.apply(ClientEvent::Disconnected {
        reason: "Herdr is not installed on the host".into(),
        ssh: Some(SshFailure::HerdrMissing),
    });
    let welcome = serde_json::from_str(include_str!(
        "../../../../sheprd-protocol/tests/fixtures/endpoint-welcome-v1.json"
    ))
    .unwrap();
    state.apply(ClientEvent::Connected(welcome));
    assert!(!state.herdr_1_missing(), "installed and connected since");
}
