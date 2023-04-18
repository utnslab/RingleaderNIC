// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{protocols::ethernet2::frame::Ethernet2Header, runtime::PacketBuf};
use std::{marker::PhantomData, ptr};

use super::pdu::ArpPdu;

#[derive(Clone, Debug)]
pub struct ArpMessage<T> {
    pub ethernet2_hdr: Ethernet2Header,
    pub arp_pdu: ArpPdu,
    pub _body_marker: PhantomData<T>,
}

impl<T> ArpMessage<T> {
    /// Creates an ARP message.
    pub fn new(header: Ethernet2Header, pdu: ArpPdu) -> Self {
        Self {
            ethernet2_hdr: header,
            arp_pdu: pdu,
            _body_marker: PhantomData,
        }
    }
}

impl<T> PacketBuf<T> for ArpMessage<T> {
    fn header_size(&self) -> usize {
        self.ethernet2_hdr.compute_size() + self.arp_pdu.compute_size()
    }

    fn body_size(&self) -> usize {
        0
    }

    fn has_body(&self) -> bool{
        return false;
    }

    unsafe fn get_body(&self) -> *mut T{
        return ptr::null_mut();
    }

    fn write_header(&self, buf: &mut [u8]) {
        let eth_hdr_size = self.ethernet2_hdr.compute_size();
        let arp_pdu_size = self.arp_pdu.compute_size();
        let mut cur_pos = 0;

        self.ethernet2_hdr
            .serialize(&mut buf[cur_pos..(cur_pos + eth_hdr_size)]);
        cur_pos += eth_hdr_size;

        self.arp_pdu
            .serialize(&mut buf[cur_pos..(cur_pos + arp_pdu_size)]);
    }

    fn take_body(self) -> Option<T> {
        None
    }

    fn write_header_index(&self, buf: &mut [u8], index:usize) {
        todo!();
    }

    unsafe fn get_batch(&self) -> *mut Vec<T>{
        todo!();
    }

    fn  if_batch(&self)  -> bool{
        return false;
    }
}
