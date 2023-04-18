// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::datagram::{Icmpv4Header, Icmpv4Type2};
use crate::{
    fail::Fail,
    protocols::{
        arp,
        ethernet2::frame::{EtherType2, Ethernet2Header},
        icmpv4::datagram::Icmpv4Message,
        ipv4::datagram::{Ipv4Header, Ipv4Protocol2},
    },
    runtime::Runtime,
    scheduler::SchedulerHandle,
};

use byteorder::{ByteOrder, NetworkEndian};

use futures::{
    channel::{
        mpsc,
        oneshot::{channel, Sender},
    },
    FutureExt, StreamExt,
};

use crate::futures_utility::UtilityMethods;

use std::{
    cell::RefCell, collections::HashMap, future::Future, net::Ipv4Addr, num::Wrapping, process,
    rc::Rc, time::Duration,
};

//==============================================================================
// ReqQueue
//==============================================================================

/// Queue of Requests
struct ReqQueue(HashMap<(u16, u16), Sender<()>>);

/// Associate Implementation for ReqQueue
impl ReqQueue {
    /// Creates an empty queue of requests.
    pub fn new() -> Self {
        Self { 0: HashMap::new() }
    }

    /// Inserts a new request in the target queue of  requests.
    pub fn insert(&mut self, req: (u16, u16), tx: Sender<()>) -> Option<Sender<()>> {
        self.0.insert(req, tx)
    }

    /// Removes a request from the target queue of requests.
    pub fn remove(&mut self, req: &(u16, u16)) -> Option<Sender<()>> {
        self.0.remove(req)
    }
}

//==============================================================================
// Icmpv4Peer
//==============================================================================

///
/// Internet Control Message Protocol (ICMP)
///
/// This is a supporting protocol for the Internet Protocol version 4 (IPv4)
/// suite. It is used by network devices to send error messages and operational
/// information indicating success or failure when communicating with other
/// peers.
///
/// ICMP for IPv4 is defined in RFC 792.
///
pub struct Icmpv4Peer<RT: Runtime> {
    /// Underlying Runtime
    rt: RT,

    /// Underlying ARP Peer
    arp: arp::Peer<RT>,

    /// Transmitter
    tx: mpsc::UnboundedSender<(Ipv4Addr, u16, u16)>,

    /// Queue of Requests
    requests: Rc<RefCell<ReqQueue>>,

    /// Sequence Number
    seq: Wrapping<u16>,

    #[allow(unused)]
    handle: SchedulerHandle,
}

impl<RT: Runtime> Icmpv4Peer<RT> {
    /// Creates a new peer for handling ICMP.
    pub fn new(rt: RT, arp: arp::Peer<RT>) -> Icmpv4Peer<RT> {
        let (tx, rx) = mpsc::unbounded();
        let requests = ReqQueue::new();
        let future = Self::background(rt.clone(), arp.clone(), rx);
        let handle = rt.spawn(future);
        Icmpv4Peer {
            rt,
            arp,
            tx,
            requests: Rc::new(RefCell::new(requests)),
            seq: Wrapping(0),
            handle,
        }
    }

    /// Background task for replying to ICMP messages.
    async fn background(
        rt: RT,
        arp: arp::Peer<RT>,
        mut rx: mpsc::UnboundedReceiver<(Ipv4Addr, u16, u16)>,
    ) {
        // Reply requests.
        while let Some((dst_ipv4_addr, id, seq_num)) = rx.next().await {
            let r: Result<_, Fail> = try {
                debug!("initiating ARP query");
                let dst_link_addr = arp.query(dst_ipv4_addr).await?;
                debug!(
                    "ARP query complete ({} -> {})",
                    dst_ipv4_addr, dst_link_addr
                );
                debug!(
                    "reply ping ({}, {}, {})",
                    dst_ipv4_addr, id, seq_num
                );
                // Send reply message.
                rt.transmit(Icmpv4Message::new(
                        Ethernet2Header::new(dst_link_addr, rt.local_link_addr(), EtherType2::Ipv4),
                        Ipv4Header::new(rt.local_ipv4_addr(), dst_ipv4_addr, Ipv4Protocol2::Icmpv4),
                        Icmpv4Header::new(Icmpv4Type2::EchoReply { id, seq_num }, 0),
                ));
            };
            if let Err(e) = r {
                warn!(
                    "reply_to_ping({}, {}, {}) failed: {:?}",
                    dst_ipv4_addr, id, seq_num, e
                )
            }
        }
    }

    /// Parses and handles a ICMP message.
    pub fn receive(&mut self, ipv4_header: &Ipv4Header, buf: RT::Buf) -> Result<(), Fail> {
        let (icmpv4_hdr, _) = Icmpv4Header::parse(buf)?;
        debug!("ICMPv4 received {:?}", icmpv4_hdr);
        match icmpv4_hdr.icmpv4_type {
            Icmpv4Type2::EchoRequest { id, seq_num } => {
                self.tx
                    .unbounded_send((ipv4_header.src_addr, id, seq_num))
                    .unwrap();
                }
            Icmpv4Type2::EchoReply { id, seq_num } => {
                if let Some(tx) = self.requests.borrow_mut().remove(&(id, seq_num)) {
                    let _ = tx.send(());
                }
            }
            _ => {
                warn!("Unsupported ICMPv4 message: {:?}", icmpv4_hdr);
            }
        }
        Ok(())
    }

    /// Computes the identifier for an ICPM message.
    fn make_id(&self) -> u16 {
        let mut state: u32 = 0xFFFF;
        let addr_octets = self.rt.local_ipv4_addr().octets();
        state += NetworkEndian::read_u16(&addr_octets[0..2]) as u32;
        state += NetworkEndian::read_u16(&addr_octets[2..4]) as u32;

        let mut pid_buf = [0u8; 4];
        NetworkEndian::write_u32(&mut pid_buf[..], process::id());
        state += NetworkEndian::read_u16(&pid_buf[0..2]) as u32;
        state += NetworkEndian::read_u16(&pid_buf[2..4]) as u32;

        let nonce: [u8; 2] = self.rt.rng_gen();
        state += NetworkEndian::read_u16(&nonce[..]) as u32;

        while state > 0xFFFF {
            state -= 0xFFFF;
        }
        !state as u16
    }

    /// Computes sequence number for an ICMP message.
    fn make_seq_num(&mut self) -> u16 {
        let Wrapping(seq_num) = self.seq;
        self.seq += Wrapping(1);
        seq_num
    }

    /// Sends a ping to a remote peer.Wrapping
    pub fn ping(
        &mut self,
        dst_ipv4_addr: Ipv4Addr,
        timeout: Option<Duration>,
    ) -> impl Future<Output = Result<Duration, Fail>> {
        let timeout = timeout.unwrap_or_else(|| Duration::from_millis(5000));
        let id = self.make_id();
        let seq_num = self.make_seq_num();
        let echo_request = Icmpv4Type2::EchoRequest { id, seq_num };
        let arp = self.arp.clone();
        let rt = self.rt.clone();
        let requests = self.requests.clone();
        async move {
            let t0 = rt.now();
            debug!("initiating ARP query");
            let dst_link_addr = arp.query(dst_ipv4_addr).await?;
            debug!(
                "ARP query complete ({} -> {})",
                dst_ipv4_addr, dst_link_addr
            );

            let msg = Icmpv4Message::new(
                Ethernet2Header::new(dst_link_addr, rt.local_link_addr(), EtherType2::Ipv4),
                Ipv4Header::new(rt.local_ipv4_addr(), dst_ipv4_addr, Ipv4Protocol2::Icmpv4),
                Icmpv4Header::new(echo_request, 0),
            );
            rt.transmit(msg);
            let rx = {
                let (tx, rx) = channel();
                assert!(requests.borrow_mut().insert((id, seq_num), tx).is_none());
                rx
            };
            // TODO: Handle cancellation here and unregister the completion in `requests`.
            let timer = rt.wait(timeout);
            let _ = rx.fuse().with_timeout(timer).await?;
            Ok(rt.now() - t0)
        }
    }
}
