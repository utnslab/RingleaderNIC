// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{runtime::Runtime, test_helpers};
use futures::task::{noop_waker_ref, Context};
use must_let::must_let;
use std::{future::Future, pin::Pin, task::Poll, time::Duration, time::Instant};

//==============================================================================
// IPv4 Ping
//==============================================================================

#[test]
fn ipv4_ping() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    let mut alice = test_helpers::new_alice2(now);

    let mut bob = test_helpers::new_bob2(now);

    // Alice pings Bob.
    let mut ping_fut = Box::pin(alice.ping(test_helpers::BOB_IPV4, None));
    must_let!(let _ = Future::poll(Pin::new(&mut ping_fut), &mut ctx));

    now += Duration::from_secs(1);
    alice.rt().advance_clock(now);
    bob.rt().advance_clock(now);

    // Bob receives ping request from Alice.
    bob.receive(alice.rt().pop_frame()).unwrap();

    // Bob replies Alice.
    bob.rt().poll_scheduler();

    now += Duration::from_secs(1);
    alice.rt().advance_clock(now);
    bob.rt().advance_clock(now);

    // Alice receives reply from Bob
    alice.receive(bob.rt().pop_frame()).unwrap();
    alice.rt().poll_scheduler();
    must_let!(let Poll::Ready(Ok(latency)) = Future::poll(Pin::new(&mut ping_fut), &mut ctx));
    assert_eq!(latency, Duration::from_secs(2));
}

#[test]
fn ipv4_ping_loop() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    let mut alice = test_helpers::new_alice2(now);

    let mut bob = test_helpers::new_bob2(now);

    for _ in 1..1000 {
        // Alice pings Bob.
        let mut ping_fut = Box::pin(alice.ping(test_helpers::BOB_IPV4, None));
        must_let!(let _ = Future::poll(Pin::new(&mut ping_fut), &mut ctx));

        now += Duration::from_secs(1);
        alice.rt().advance_clock(now);
        bob.rt().advance_clock(now);

        // Bob receives ping request from Alice.
        bob.receive(alice.rt().pop_frame()).unwrap();

        // Bob replies Alice.
        bob.rt().poll_scheduler();

        now += Duration::from_secs(1);
        alice.rt().advance_clock(now);
        bob.rt().advance_clock(now);

        // Alice receives reply from Bob
        alice.receive(bob.rt().pop_frame()).unwrap();
        alice.rt().poll_scheduler();
        must_let!(let Poll::Ready(Ok(latency)) = Future::poll(Pin::new(&mut ping_fut), &mut ctx));
        assert_eq!(latency, Duration::from_secs(2));
    }
}
