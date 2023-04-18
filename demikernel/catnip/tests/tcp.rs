// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#![feature(new_uninit)]
#![feature(const_panic, const_alloc_layout)]
#![feature(const_mut_refs, const_type_name)]
#![feature(maybe_uninit_uninit_array, maybe_uninit_extra, maybe_uninit_ref)]

use catnip::{
    fail::Fail,
    interop::dmtr_opcode_t,
    libos::LibOS,
    protocols::{ip, ipv4},
    runtime::Runtime,
};

use crossbeam_channel::{self};

use libc;

use std::{convert::TryFrom, net::Ipv4Addr, thread};

mod common;
use common::libos::*;
use common::runtime::*;
use common::*;

//==============================================================================
// Open/Close Passive Socket
//==============================================================================

/// Tests if a passive socket may be successfully opened and closed.
fn do_tcp_connection_setup(libos: &mut LibOS<DummyRuntime>, port: u16) {
    let port = ip::Port::try_from(port).unwrap();
    let local = ipv4::Endpoint::new(ALICE_IPV4, port);

    // Open and close a connection.
    let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
    libos.bind(sockfd, local).unwrap();
    libos.listen(sockfd, 8).unwrap();
    libos.close(sockfd).unwrap();
}

#[test]
fn catnip_tcp_connection_setup() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, tx, rx, arp());

    do_tcp_connection_setup(&mut libos, PORT_BASE);
}

//==============================================================================
// Establish Connection
//==============================================================================

/// Tests if data can be successfully established.
fn do_tcp_establish_connection(port: u16) {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        libos.listen(sockfd, 8).unwrap();
        let qt = libos.accept(sockfd).unwrap();
        let r = libos.wait(qt);
        assert_eq!(r.qr_opcode, dmtr_opcode_t::DMTR_OPC_ACCEPT);
        let qd = unsafe { r.qr_value.ares.qd } as u32;

        // Close connection.
        libos.close(qd).unwrap();
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = DummyLibOS::new(BOB_MAC, BOB_IPV4, bob_tx, alice_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}

#[test]
fn catnip_tcp_establish_connection() {
    do_tcp_establish_connection(PORT_BASE + 1)
}

//==============================================================================
// Push
//==============================================================================

/// Tests if data can be successfully established.
fn do_tcp_push_remote(port: u16) {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        libos.listen(sockfd, 8).unwrap();
        let qt = libos.accept(sockfd).unwrap();
        let r = libos.wait(qt);
        assert_eq!(r.qr_opcode, dmtr_opcode_t::DMTR_OPC_ACCEPT);

        // Pop data.
        let qd = unsafe { r.qr_value.ares.qd } as u32;
        let qt = libos.pop(qd).unwrap();
        let qr = libos.wait(qt);
        assert_eq!(qr.qr_opcode, dmtr_opcode_t::DMTR_OPC_POP);

        // Sanity check data.
        let sga = unsafe { qr.qr_value.sga };
        DummyLibOS::check_data(sga);
        libos.rt().free_sgarray(sga);

        // Close connection.
        libos.close(qd).unwrap();
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = DummyLibOS::new(BOB_MAC, BOB_IPV4, bob_tx, alice_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);

        // Cook some data.
        let body_sga = DummyLibOS::cook_data(&mut libos, 32);

        // Push data.
        let qt = libos.push(sockfd, &body_sga).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_PUSH);
        libos.rt().free_sgarray(body_sga);

        // Close connection.
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}

#[test]
fn catnip_tcp_push_remote() {
    do_tcp_push_remote(PORT_BASE + 2)
}

//==============================================================================
// Bad Socket
//==============================================================================

/// Tests for bad socket creation.
fn do_tcp_bad_socket() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, tx, rx, arp());

    let domains: Vec<libc::c_int> = vec![
        libc::AF_ALG,
        libc::AF_APPLETALK,
        libc::AF_ASH,
        libc::AF_ATMPVC,
        libc::AF_ATMSVC,
        libc::AF_AX25,
        libc::AF_BLUETOOTH,
        libc::AF_BRIDGE,
        libc::AF_CAIF,
        libc::AF_CAN,
        libc::AF_DECnet,
        libc::AF_ECONET,
        libc::AF_IB,
        libc::AF_IEEE802154,
        // libc::AF_INET,
        libc::AF_INET6,
        libc::AF_IPX,
        libc::AF_IRDA,
        libc::AF_ISDN,
        libc::AF_IUCV,
        libc::AF_KEY,
        libc::AF_LLC,
        libc::AF_LOCAL,
        libc::AF_MPLS,
        libc::AF_NETBEUI,
        libc::AF_NETLINK,
        libc::AF_NETROM,
        libc::AF_NFC,
        libc::AF_PACKET,
        libc::AF_PHONET,
        libc::AF_PPPOX,
        libc::AF_RDS,
        libc::AF_ROSE,
        libc::AF_ROUTE,
        libc::AF_RXRPC,
        libc::AF_SECURITY,
        libc::AF_SNA,
        libc::AF_TIPC,
        libc::AF_UNIX,
        libc::AF_UNSPEC,
        libc::AF_VSOCK,
        libc::AF_WANPIPE,
        libc::AF_X25,
        libc::AF_XDP,
    ];

    let scoket_types: Vec<libc::c_int> = vec![
        libc::SOCK_DCCP,
        // libc::SOCK_DGRAM,
        libc::SOCK_PACKET,
        libc::SOCK_RAW,
        libc::SOCK_RDM,
        libc::SOCK_SEQPACKET,
        // libc::SOCK_STREAM,
    ];

    // Invalid domain.
    for d in domains {
        let sockfd = libos.socket(d, libc::SOCK_STREAM, 0);
        let e = sockfd.unwrap_err();
        assert_eq!(e, (Fail::AddressFamilySupport {}));
    }

    // Invalid socket tpe.
    for t in scoket_types {
        let sockfd = libos.socket(libc::AF_INET, t, 0);
        let e = sockfd.unwrap_err();
        assert_eq!(e, (Fail::SocketTypeSupport {}));
    }
}

#[test]
fn catnip_tcp_bad_socket() {
    do_tcp_bad_socket()
}

//==============================================================================
// Bad Bind
//==============================================================================

/// Test bad calls for `bind()`.
fn do_tcp_bad_bind(port: u16) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, tx, rx, arp());

    // Invalid file descriptor.
    let port = ip::Port::try_from(port).unwrap();
    let local = ipv4::Endpoint::new(ALICE_IPV4, port);
    let e = libos.bind(0, local).unwrap_err();
    assert_eq!(e, (Fail::BadFileDescriptor {}));
}

#[test]
fn catnip_tcp_bad_bind() {
    do_tcp_bad_bind(PORT_BASE + 3);
}

//==============================================================================
// Bad Listen
//==============================================================================

/// Tests bad calls for `listen()`.
fn do_tcp_bad_listen(port: u16) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, tx, rx, arp());

    let port = ip::Port::try_from(port).unwrap();
    let local = ipv4::Endpoint::new(ALICE_IPV4, port);

    // Invalid file descriptor.
    let e = libos.listen(0, 8).unwrap_err();
    assert_eq!(e, (Fail::BadFileDescriptor {}));

    // Invalid backlog length
    let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
    libos.bind(sockfd, local).unwrap();
    let e = libos.listen(sockfd, 0).unwrap_err();
    assert_eq!(
        e,
        (Fail::Invalid {
            details: "backlog length"
        })
    );
    libos.close(sockfd).unwrap();
}

#[test]
fn catnip_tcp_bad_listen() {
    do_tcp_bad_listen(PORT_BASE + 4);
}

//==============================================================================
// Bad Accept
//==============================================================================

/// Tests bad calls for `accept()`.
fn do_tcp_bad_accept() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, tx, rx, arp());

    // Invalid file descriptor.
    let e = libos.accept(0).unwrap_err();
    assert_eq!(e, (Fail::BadFileDescriptor {}));
}

#[test]
fn catnip_tcp_bad_accept() {
    do_tcp_bad_accept();
}

//==============================================================================
// Bad Accept
//==============================================================================

/// Tests if data can be successfully established.
fn do_tcp_bad_connect(port: u16) {
    let (alice_tx, alice_rx) = crossbeam_channel::unbounded();
    let (bob_tx, bob_rx) = crossbeam_channel::unbounded();

    let alice = thread::spawn(move || {
        let mut libos = DummyLibOS::new(ALICE_MAC, ALICE_IPV4, alice_tx, bob_rx, arp());
        let port = ip::Port::try_from(port).unwrap();
        let local = ipv4::Endpoint::new(ALICE_IPV4, port);

        // Open connection.
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        libos.bind(sockfd, local).unwrap();
        libos.listen(sockfd, 8).unwrap();
        let qt = libos.accept(sockfd).unwrap();
        let r = libos.wait(qt);
        assert_eq!(r.qr_opcode, dmtr_opcode_t::DMTR_OPC_ACCEPT);
        let qd = unsafe { r.qr_value.ares.qd } as u32;

        // Close connection.
        libos.close(qd).unwrap();
        libos.close(sockfd).unwrap();
    });

    let bob = thread::spawn(move || {
        let mut libos = DummyLibOS::new(BOB_MAC, BOB_IPV4, bob_tx, alice_rx, arp());

        let port = ip::Port::try_from(port).unwrap();
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);

        println!("BAD FD");
        // Bad file descriptor.
        let e = libos.connect(0, remote).unwrap_err();
        assert_eq!(e, (Fail::BadFileDescriptor {}));

        println!("BAD endpoint");

        // Bad endpoint.
        let remote = ipv4::Endpoint::new(Ipv4Addr::new(0, 0, 0, 0), port);
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_FAILED);

        // Close connection.
        let remote = ipv4::Endpoint::new(ALICE_IPV4, port);
        let sockfd = libos.socket(libc::AF_INET, libc::SOCK_STREAM, 0).unwrap();
        let qt = libos.connect(sockfd, remote).unwrap();
        assert_eq!(libos.wait(qt).qr_opcode, dmtr_opcode_t::DMTR_OPC_CONNECT);
        libos.close(sockfd).unwrap();
    });

    alice.join().unwrap();
    bob.join().unwrap();
}

#[test]
fn catnip_tcp_bad_connect() {
    do_tcp_bad_connect(PORT_BASE + 5)
}
