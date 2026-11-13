//! The tunnel over a Tokio stream, as the daemon runs it.

use super::*;
use crate::{
    Error, Initiator, Responder,
    stream::{accept, connect},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

#[tokio::test]
async fn bytes_cross_a_tunnel_both_ways_and_close_cleanly() {
    let (phone, computer) = (keys(1), keys(2));
    let (dialer_side, listener_side) = duplex(4_096);
    let listener = tokio::spawn(async move {
        let computer = keys(2);
        let paired = keys(1).identity().x25519();
        let responder = Responder::session(&computer, &ROUTE).unwrap();
        accept(listener_side, &computer, responder, |remote| {
            *remote == paired
        })
        .await
    });
    let identity = computer.identity();
    let (initiator, first) = Initiator::session(&phone, &identity, &ROUTE).unwrap();
    let mut dialing = connect(dialer_side, initiator, &first, identity.x25519())
        .await
        .unwrap();
    let mut listening = listener.await.unwrap().unwrap();
    assert_eq!(dialing.hash, listening.hash);
    assert_eq!(listening.remote, phone.identity().x25519());

    // More than one record's worth, to cross record boundaries.
    let big = vec![0x42; 100_000];
    let writer = {
        let big = big.clone();
        tokio::spawn(async move {
            dialing.io.write_all(&big).await.unwrap();
            dialing.io.shutdown().await.unwrap();
            dialing
        })
    };
    let mut received = Vec::new();
    listening.io.read_to_end(&mut received).await.unwrap();
    assert_eq!(received, big);
    let dialing = writer.await.unwrap();
    drop(dialing.io);
    assert!(listening.task.await.unwrap().is_ok());
}

#[tokio::test]
async fn an_unpaired_dialer_is_refused_and_hears_nothing() {
    let (stranger, computer) = (keys(9), keys(2));
    let (dialer_side, listener_side) = duplex(4_096);
    let listener = tokio::spawn(async move {
        let computer = keys(2);
        let responder = Responder::session(&computer, &ROUTE).unwrap();
        accept(listener_side, &computer, responder, |_| false).await
    });
    let identity = computer.identity();
    let (initiator, first) = Initiator::session(&stranger, &identity, &ROUTE).unwrap();
    let listener_key = identity.x25519();
    let dialing =
        tokio::spawn(async move { connect(dialer_side, initiator, &first, listener_key).await });
    assert!(matches!(listener.await.unwrap(), Err(Error::Refused)));
    // The listener dropped its side without answering.
    assert!(matches!(dialing.await.unwrap(), Err(Error::Closed)));
}
