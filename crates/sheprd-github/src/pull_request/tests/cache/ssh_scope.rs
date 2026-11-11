use super::*;

#[test]
fn ssh_answers_are_fenced_after_host_account_or_connection_changes() {
    let now = Instant::now();
    let origin = Origin::Ssh("box".into());
    for (next_origin, login, generation) in [
        (Origin::Ssh("other".into()), "fixture", 1),
        (origin.clone(), "other-account", 1),
        (origin.clone(), "fixture", 2),
    ] {
        let mut peer = Peer::new();
        let scope = (0, 1, String::new());
        let tokens = tokens();
        assert!(
            peer.cache
                .scope(scope.clone(), tokens.clone(), "fixture", origin.clone())
        );
        assert!(!peer.cache.scope(scope, tokens, "fixture", origin.clone()));
        let input = Input {
            checkout: Some("/work/checkout".into()),
            ..input("feature")
        };
        peer.cache.schedule([input.clone()], now);
        peer.cache.poll(now);
        let (old, requested, host, _, _) = peer.incoming.try_recv().unwrap();
        assert_eq!(host, origin);
        assert_eq!(requested, [input.clone()]);
        let profile = crate::Profile::fixture(login, Arc::new(login.into()), &[]);
        assert!(peer.cache.scope(
            (0, generation, String::new()),
            crate::Tokens::new(&profile, []),
            login,
            next_origin
        ));
        peer.outgoing
            .send((old, answered(input.clone(), Ok(Some(fixture().unwrap())))))
            .unwrap();
        peer.cache.poll(now);
        assert!(peer.cache.get(&input).is_none());
        peer.cache.schedule([input.clone()], now);
        peer.cache.poll(now);
        peer.complete(now, Ok(Some(fixture().unwrap())), None);
        assert!(peer.cache.get(&input).is_some());
    }
}
