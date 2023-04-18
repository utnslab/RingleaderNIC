// // Copyright (c) Microsoft Corporation.
// // Licensed under the MIT license.

use crate::{
    collections::bytes::BytesMut,
    fail::Fail,
    file_table::FileDescriptor,
    protocols::{ip, ipv4},
    test_helpers,
};
use futures::task::{noop_waker_ref, Context};
use must_let::must_let;
use std::{
    convert::TryFrom,
    future::Future,
    pin::Pin,
    task::Poll,
    time::{Duration, Instant},
};

//==============================================================================
// Bind & Close
//==============================================================================

#[test]
fn udp_bind_close() {
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = ipv4::Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: FileDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();

    now += Duration::from_micros(1);

    // Close peers.
    alice.close(alice_fd).unwrap();
    bob.close(bob_fd).unwrap();
}

//==============================================================================
// Push & Pop
//==============================================================================

#[test]
fn udp_push_pop() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = ipv4::Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: FileDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();

    // Send data to Bob.
    let buf = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    alice.udp_pushto(alice_fd, buf.clone(), bob_addr).unwrap();
    alice.rt().poll_scheduler();

    now += Duration::from_micros(1);

    // Receive data from Alice.
    bob.receive(alice.rt().pop_frame()).unwrap();
    let mut pop_future = bob.udp_pop(bob_fd);
    must_let!(let Poll::Ready(Ok((Some(remote_addr), received_buf))) = Future::poll(Pin::new(&mut pop_future), &mut ctx));
    assert_eq!(remote_addr, alice_addr);
    assert_eq!(received_buf, buf);

    // Close peers.
    alice.close(alice_fd).unwrap();
    bob.close(bob_fd).unwrap();
}

//==============================================================================
// Ping Pong
//==============================================================================

#[test]
fn udp_ping_pong() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = ipv4::Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: FileDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();

    // Send data to Bob.
    let buf_a = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    alice.udp_pushto(alice_fd, buf_a.clone(), bob_addr).unwrap();
    alice.rt().poll_scheduler();

    now += Duration::from_micros(1);

    // Receive data from Alice.
    bob.receive(alice.rt().pop_frame()).unwrap();
    let mut pop_future = bob.udp_pop(bob_fd);
    must_let!(let Poll::Ready(Ok((Some(remote_addr), received_buf_a))) = Future::poll(Pin::new(&mut pop_future), &mut ctx));
    assert_eq!(remote_addr, alice_addr);
    assert_eq!(received_buf_a, buf_a);

    now += Duration::from_micros(1);

    // Send data to Alice.
    let buf_b = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    bob.udp_pushto(bob_fd, buf_b.clone(), alice_addr).unwrap();
    bob.rt().poll_scheduler();

    now += Duration::from_micros(1);

    // Receive data from Bob.
    alice.receive(bob.rt().pop_frame()).unwrap();
    let mut pop_future = alice.udp_pop(alice_fd);
    must_let!(let Poll::Ready(Ok((Some(remote_addr), received_buf_b))) = Future::poll(Pin::new(&mut pop_future), &mut ctx));
    assert_eq!(remote_addr, bob_addr);
    assert_eq!(received_buf_b, buf_b);

    // Close peers.
    alice.close(alice_fd).unwrap();
    bob.close(bob_fd).unwrap();
}

//==============================================================================
// Loop Bind & Close
//==============================================================================

#[test]
fn udp_loop1_bind_close() {
    // Loop.
    for _ in 0..1000 {
        udp_bind_close();
    }
}

#[test]
fn udp_loop2_bind_close() {
    let mut now = Instant::now();

    // Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);

    // Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = ipv4::Endpoint::new(test_helpers::BOB_IPV4, bob_port);

    // Loop.
    for _ in 0..1000 {
        // Bind Alice.
        let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
        alice.udp_bind(alice_fd, alice_addr).unwrap();

        // Bind bob.
        let bob_fd: FileDescriptor = bob.udp_socket().unwrap();
        bob.udp_bind(bob_fd, bob_addr).unwrap();

        now += Duration::from_micros(1);

        // Close peers.
        alice.close(alice_fd).unwrap();
        bob.close(bob_fd).unwrap();
    }
}

//==============================================================================
// Loop Push & Pop
//==============================================================================

#[test]
fn udp_loop1_push_pop() {
    // Loop.
    for _ in 0..1000 {
        udp_push_pop();
    }
}

#[test]
fn udp_loop2_push_pop() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = ipv4::Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: FileDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();
    // Loop.
    for b in 0..1000 {
        // Send data to Bob.
        let buf = BytesMut::from(&vec![(b % 256) as u8; 32][..]).freeze();
        alice.udp_pushto(alice_fd, buf.clone(), bob_addr).unwrap();
        alice.rt().poll_scheduler();

        now += Duration::from_micros(1);

        // Receive data from Alice.
        bob.receive(alice.rt().pop_frame()).unwrap();
        let mut pop_future = bob.udp_pop(bob_fd);
        must_let!(let Poll::Ready(Ok((Some(remote_addr), received_buf))) = Future::poll(Pin::new(&mut pop_future), &mut ctx));
        assert_eq!(remote_addr, alice_addr);
        assert_eq!(received_buf, buf);
    }

    // Close peers.
    alice.close(alice_fd).unwrap();
    bob.close(bob_fd).unwrap();
}

//==============================================================================
// Loop Ping Pong
//==============================================================================

#[test]
fn udp_loop1_ping_pong() {
    // Loop.
    for _ in 0..1000 {
        udp_ping_pong();
    }
}

#[test]
fn udp_loop2_ping_pong() {
    let mut ctx = Context::from_waker(noop_waker_ref());
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = ipv4::Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: FileDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();
    //
    // Loop.
    for _ in 0..1000 {
        // Send data to Bob.
        let buf_a = BytesMut::from(&vec![0x5a; 32][..]).freeze();
        alice.udp_pushto(alice_fd, buf_a.clone(), bob_addr).unwrap();
        alice.rt().poll_scheduler();

        now += Duration::from_micros(1);

        // Receive data from Alice.
        bob.receive(alice.rt().pop_frame()).unwrap();
        let mut pop_future = bob.udp_pop(bob_fd);
        must_let!(let Poll::Ready(Ok((Some(remote_addr), received_buf_a))) = Future::poll(Pin::new(&mut pop_future), &mut ctx));
        assert_eq!(remote_addr, alice_addr);
        assert_eq!(received_buf_a, buf_a);

        now += Duration::from_micros(1);

        // Send data to Alice.
        let buf_b = BytesMut::from(&vec![0x5a; 32][..]).freeze();
        bob.udp_pushto(bob_fd, buf_b.clone(), alice_addr).unwrap();
        bob.rt().poll_scheduler();

        now += Duration::from_micros(1);

        // Receive data from Bob.
        alice.receive(bob.rt().pop_frame()).unwrap();
        let mut pop_future = alice.udp_pop(alice_fd);
        must_let!(let Poll::Ready(Ok((Some(remote_addr), received_buf_b))) = Future::poll(Pin::new(&mut pop_future), &mut ctx));
        assert_eq!(remote_addr, bob_addr);
        assert_eq!(received_buf_b, buf_b);
    }

    // Close peers.
    alice.close(alice_fd).unwrap();
    bob.close(bob_fd).unwrap();
}

//==============================================================================
// Bad Bind
//==============================================================================

#[test]
fn udp_bind_address_in_use() {
    let now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Try to bind Alice again.
    must_let!(let Err(err) = alice.udp_bind(alice_fd, alice_addr));
    assert_eq!(
        err,
        Fail::Malformed {
            details: "Port already listening",
        }
    );

    // Close peers.
    alice.close(alice_fd).unwrap();
}

#[test]
fn udp_bind_bad_file_descriptor() {
    let now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = u32::MAX;

    // Try to bind Alice.
    must_let!(let Err(err) = alice.udp_bind(alice_fd, alice_addr));
    assert_eq!(err, Fail::BadFileDescriptor {});
}

//==============================================================================
// Bad Close
//==============================================================================

#[test]
fn udp_close_bad_file_descriptor() {
    let now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();

    // Try to close bad file descriptor.
    must_let!(let Err(err) = alice.close(u32::MAX));
    assert_eq!(err, Fail::BadFileDescriptor {});

    // Try to close Alice two times.
    alice.close(alice_fd).unwrap();
    must_let!(let Err(err) = alice.close(alice_fd));
    assert_eq!(err, Fail::BadFileDescriptor {});
}

//==============================================================================
// Bad Pop
//==============================================================================

#[test]
fn udp_pop_not_bound() {
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = ipv4::Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: FileDescriptor = bob.udp_socket().unwrap();

    // Send data to Bob.
    let buf = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    alice.udp_pushto(alice_fd, buf.clone(), bob_addr).unwrap();
    alice.rt().poll_scheduler();

    now += Duration::from_micros(1);

    // Receive data from Alice.
    must_let!(let Err(err) = bob.receive(alice.rt().pop_frame()));
    assert_eq!(
        err,
        Fail::Malformed {
            details: "Port not bound",
        }
    );

    // Close peers.
    alice.close(alice_fd).unwrap();
    bob.close(bob_fd).unwrap();
}

//==============================================================================
// Bad Push
//==============================================================================

#[test]
fn udp_push_bad_file_descriptor() {
    let mut now = Instant::now();

    // Setup Alice.
    let mut alice = test_helpers::new_alice2(now);
    let alice_port = ip::Port::try_from(80).unwrap();
    let alice_addr = ipv4::Endpoint::new(test_helpers::ALICE_IPV4, alice_port);
    let alice_fd: FileDescriptor = alice.udp_socket().unwrap();
    alice.udp_bind(alice_fd, alice_addr).unwrap();

    // Setup Bob.
    let mut bob = test_helpers::new_bob2(now);
    let bob_port = ip::Port::try_from(80).unwrap();
    let bob_addr = ipv4::Endpoint::new(test_helpers::BOB_IPV4, bob_port);
    let bob_fd: FileDescriptor = bob.udp_socket().unwrap();
    bob.udp_bind(bob_fd, bob_addr).unwrap();

    // Send data to Bob.
    let buf = BytesMut::from(&vec![0x5a; 32][..]).freeze();
    must_let!(let Err(err) = alice.udp_pushto(u32::MAX, buf.clone(), bob_addr));
    assert_eq!(err, Fail::BadFileDescriptor {});

    alice.rt().poll_scheduler();
    now += Duration::from_micros(1);

    // Close peers.
    alice.close(alice_fd).unwrap();
    bob.close(bob_fd).unwrap();
}
