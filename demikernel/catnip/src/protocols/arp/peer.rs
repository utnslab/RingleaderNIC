// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::{
    cache::ArpCache,
    msg::ArpMessage,
    options::ArpOptions,
    pdu::{ArpOperation, ArpPdu},
};
use crate::futures_utility::UtilityMethods;
use crate::{
    fail::Fail,
    protocols::ethernet2::{
        frame::{EtherType2, Ethernet2Header},
        MacAddress,
    },
    runtime::Runtime,
    scheduler::SchedulerHandle,
};
use futures::{
    channel::oneshot::{channel, Receiver, Sender},
    FutureExt,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    net::Ipv4Addr,
    rc::Rc,
    time::{Duration, Instant},
};

///
/// Arp Peer
/// - TODO: Allow multiple waiters for the same address
#[derive(Clone)]
pub struct ArpPeer<RT: Runtime> {
    rt: RT,
    cache: Rc<RefCell<ArpCache>>,
    background: Rc<SchedulerHandle>,
    waiters: Rc<RefCell<HashMap<Ipv4Addr, Sender<MacAddress>>>>,
    options: ArpOptions,
}

impl<RT: Runtime> ArpPeer<RT> {
    pub fn new(now: Instant, rt: RT, options: ArpOptions) -> Result<ArpPeer<RT>, Fail> {
        let cache = Rc::new(RefCell::new(ArpCache::new(
            now,
            Some(options.cache_ttl),
            Some(&options.initial_values),
            options.disable_arp,
        )));

        let handle = rt.spawn(Self::background(rt.clone(), cache.clone()));
        let peer = ArpPeer {
            rt,
            cache,
            background: Rc::new(handle),
            waiters: Rc::new(RefCell::new(HashMap::default())),
            options,
        };

        Ok(peer)
    }

    /// Drops a waiter for a target IP address.
    fn do_drop(&mut self, ipv4_addr: Ipv4Addr) {
        self.waiters.borrow_mut().remove(&ipv4_addr);
    }

    fn do_insert(&mut self, ipv4_addr: Ipv4Addr, link_addr: MacAddress) -> Option<MacAddress> {
        if let Some(sender) = self.waiters.borrow_mut().remove(&ipv4_addr) {
            let _ = sender.send(link_addr);
        }
        self.cache.borrow_mut().insert(ipv4_addr, link_addr)
    }

    fn do_wait_link_addr(&mut self, ipv4_addr: Ipv4Addr) -> impl Future<Output = MacAddress> {
        let (tx, rx): (Sender<MacAddress>, Receiver<MacAddress>) = channel();
        if let Some(&link_addr) = self.cache.borrow().get(ipv4_addr) {
            let _ = tx.send(link_addr);
        } else {
            assert!(
                self.waiters.borrow_mut().insert(ipv4_addr, tx).is_none(),
                "Duplicate waiter for {:?}",
                ipv4_addr
            );
        }
        rx.map(|r| r.expect("Dropped waiter?"))
    }

    /// Background task that cleans up the ARP cache from time to time.
    async fn background(rt: RT, cache: Rc<RefCell<ArpCache>>) {
        loop {
            let current_time = rt.now();
            {
                let mut cache = cache.borrow_mut();
                cache.advance_clock(current_time);
                // TODO: re-enable eviction once TCP/IP stack is fully functional.
                // cache.clear();
            }
            rt.wait(Duration::from_secs(1)).await;
        }
    }

    pub fn receive(&mut self, buf: RT::Buf) -> Result<(), Fail> {
        // from RFC 826:
        // > ?Do I have the hardware type in ar$hrd?
        // > [optionally check the hardware length ar$hln]
        // > ?Do I speak the protocol in ar$pro?
        // > [optionally check the protocol length ar$pln]
        let pdu = ArpPdu::parse(buf)?;
        debug!("Received {:?}", pdu);

        // from RFC 826:
        // > Merge_flag := false
        // > If the pair <protocol type, sender protocol address> is
        // > already in my translation table, update the sender
        // > hardware address field of the entry with the new
        // > information in the packet and set Merge_flag to true.
        let merge_flag = {
            if self.cache.borrow().get(pdu.sender_protocol_addr).is_some() {
                self.do_insert(pdu.sender_protocol_addr, pdu.sender_hardware_addr);
                true
            } else {
                false
            }
        };
        // from RFC 826: ?Am I the target protocol address?
        if pdu.target_protocol_addr != self.rt.local_ipv4_addr() {
            if merge_flag {
                // we did do something.
                return Ok(());
            } else {
                // we didn't do anything.
                return Err(Fail::Ignored {
                    details: "unrecognized IP address",
                });
            }
        }
        // from RFC 826:
        // > If Merge_flag is false, add the triplet <protocol type,
        // > sender protocol address, sender hardware address> to
        // > the translation table.
        if !merge_flag {
            self.do_insert(pdu.sender_protocol_addr, pdu.sender_hardware_addr);
        }

        match pdu.operation {
            ArpOperation::Request => {
                // from RFC 826:
                // > Swap hardware and protocol fields, putting the local
                // > hardware and protocol addresses in the sender fields.
                let reply = ArpMessage::new(
                    Ethernet2Header {
                        dst_addr: pdu.sender_hardware_addr,
                        src_addr: self.rt.local_link_addr(),
                        ether_type: EtherType2::Arp,
                    },
                    ArpPdu::new(
                        ArpOperation::Reply,
                        self.rt.local_link_addr(),
                        self.rt.local_ipv4_addr(),
                        pdu.sender_hardware_addr,
                        pdu.sender_protocol_addr,
                    ),
                );
                debug!("Responding {:?}", reply);
                self.rt.transmit(reply);
                Ok(())
            }
            ArpOperation::Reply => {
                debug!(
                    "reply from `{}/{}`",
                    pdu.sender_protocol_addr, pdu.sender_hardware_addr
                );
                self.cache
                    .borrow_mut()
                    .insert(pdu.sender_protocol_addr, pdu.sender_hardware_addr);
                Ok(())
            }
        }
    }

    pub fn try_query(&self, ipv4_addr: Ipv4Addr) -> Option<MacAddress> {
        self.cache.borrow().get(ipv4_addr).cloned()
    }

    pub fn query(&self, ipv4_addr: Ipv4Addr) -> impl Future<Output = Result<MacAddress, Fail>> {
        let rt = self.rt.clone();
        let mut arp = self.clone();
        let cache = self.cache.clone();
        let arp_options = self.options.clone();
        async move {
            if let Some(&link_addr) = cache.borrow().get(ipv4_addr) {
                return Ok(link_addr);
            }
            let msg = ArpMessage::new(
                Ethernet2Header {
                    dst_addr: MacAddress::broadcast(),
                    src_addr: rt.local_link_addr(),
                    ether_type: EtherType2::Arp,
                },
                ArpPdu::new(
                    ArpOperation::Request,
                    rt.local_link_addr(),
                    rt.local_ipv4_addr(),
                    MacAddress::broadcast(),
                    ipv4_addr,
                ),
            );
            let mut arp_response = arp.do_wait_link_addr(ipv4_addr).fuse();

            // from TCP/IP illustrated, chapter 4:
            // > The frequency of the ARP request is very close to one per
            // > second, the maximum suggested by [RFC1122].
            let result = {
                for i in 0..arp_options.retry_count + 1 {
                    rt.transmit(msg.clone());
                    let timer = rt.wait(arp_options.request_timeout);

                    match arp_response.with_timeout(timer).await {
                        Ok(link_addr) => {
                            debug!("ARP result available ({})", link_addr);
                            return Ok(link_addr);
                        }
                        Err(_) => {
                            warn!("ARP request timeout; attempt {}.", i + 1);
                        }
                    }
                }
                Err(Fail::Timeout {})
            };

            arp.do_drop(ipv4_addr);

            result
        }
    }

    pub fn export_cache(&self) -> HashMap<Ipv4Addr, MacAddress> {
        self.cache.borrow().export()
    }
}
