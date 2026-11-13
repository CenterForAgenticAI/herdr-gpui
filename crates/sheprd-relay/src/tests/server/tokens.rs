//! Listen tokens, one per person or machine, each revoked on its own.

use super::*;
use crate::hub::Access;
use std::net::IpAddr;

const ALICE: &str = "alice's token, long enough";
const BOB: &str = "bob's token, also long enough";

fn file(tokens: &[&str]) -> String {
    let mut text = String::from("# Listen tokens\n\n");
    for (index, token) in tokens.iter().enumerate() {
        text.push_str(&format!("person {index}: {token}\n"));
    }
    text
}

#[test]
fn each_token_in_the_file_admits_and_others_do_not() {
    let access = Access::tokens(&file(&[ALICE, BOB])).unwrap();
    assert!(access.requires_token());
    let alice = access.admit(Some(ALICE.as_bytes())).unwrap();
    let bob = access.admit(Some(BOB.as_bytes())).unwrap();
    assert!(alice.is_some() && bob.is_some());
    assert_ne!(alice, bob);
    assert!(access.admit(Some(b"a token nobody holds")).is_none());
    assert!(access.admit(None).is_none());
    // The token alone works too, and `#` inside one is part of it.
    let bare = Access::tokens("a #token with a hash in it\n").unwrap();
    assert!(bare.admit(Some(b"a #token with a hash in it")).is_some());
    // An open relay admits anyone, with no token to revoke.
    assert_eq!(Access::Open.admit(None), Some(None));
}

#[test]
fn a_short_token_or_an_empty_file_refuses_the_whole_file() {
    assert!(Access::tokens(&file(&[ALICE, "short"])).is_err());
    assert!(Access::tokens("# nothing but comments\n").is_err());
    let access = Access::tokens(&file(&[ALICE])).unwrap();
    // A bad reload keeps the tokens already loaded.
    assert!(access.replace("short").is_err());
    assert!(access.admit(Some(ALICE.as_bytes())).is_some());
}

#[tokio::test]
async fn revoking_one_token_closes_only_its_listeners() {
    let hub = Arc::new(Hub::new(
        Limits::default(),
        Access::tokens(&file(&[ALICE, BOB])).unwrap(),
    ));
    let address: IpAddr = "198.51.100.90".parse().unwrap();
    let alice = hub.access.admit(Some(ALICE.as_bytes())).unwrap();
    let bob = hub.access.admit(Some(BOB.as_bytes())).unwrap();
    let alices_route = RouteId::of(signing_key(160).verifying_key());
    let bobs_route = RouteId::of(signing_key(161).verifying_key());
    let mut alices = hub.register_with(alices_route, address, alice).unwrap();
    let _bobs = hub.register_with(bobs_route, address, bob).unwrap();
    let kept = hub.access.replace(&file(&[BOB])).unwrap();
    hub.keep_tokens(&kept);
    assert!(hub.access.admit(Some(ALICE.as_bytes())).is_none());
    // Alice's listener is told: its channel closes.
    let closed = tokio::time::timeout(Duration::from_secs(1), alices.incoming.recv()).await;
    assert!(matches!(closed, Ok(None)));
    assert_eq!(hub.metrics.read(crate::metrics::Gauge::Listeners), 1);
}

#[tokio::test]
async fn revoking_a_token_ends_the_sessions_its_listener_carries() {
    let hub = Arc::new(Hub::new(
        Limits::default(),
        Access::tokens(&file(&[ALICE, BOB])).unwrap(),
    ));
    let alice = hub.access.admit(Some(ALICE.as_bytes())).unwrap();
    let route = RouteId::of(signing_key(162).verifying_key());
    let address: IpAddr = "198.51.100.91".parse().unwrap();
    let registration = hub.register_with(route, address, alice).unwrap();
    registration.set_dial_keys(&[public(&phone())]);
    let waiting = hub
        .dial(route, public(&phone()), "198.51.100.92".parse().unwrap())
        .unwrap()
        .unwrap();
    let session = hub.open(waiting.slot).unwrap();
    let kept = hub.access.replace(&file(&[BOB])).unwrap();
    hub.keep_tokens(&kept);
    tokio::time::timeout(Duration::from_secs(1), session.revoked())
        .await
        .expect("the session outlived its token");
}

#[tokio::test]
async fn a_token_revoked_after_its_check_cannot_register() {
    let hub = Arc::new(Hub::new(
        Limits::default(),
        Access::tokens(&file(&[ALICE, BOB])).unwrap(),
    ));
    // Checked, then revoked before the listener registers.
    let alice = hub.access.admit(Some(ALICE.as_bytes())).unwrap();
    hub.access.replace(&file(&[BOB])).unwrap();
    let route = RouteId::of(signing_key(163).verifying_key());
    let refused = hub.register_with(route, "198.51.100.93".parse().unwrap(), alice);
    assert_eq!(refused.err(), Some(crate::hub::ListenRefusal::Revoked));
}
