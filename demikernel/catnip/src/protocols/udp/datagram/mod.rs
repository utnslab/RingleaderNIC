// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod header;

use std::intrinsics::transmute;

use crate::{
    protocols::{ethernet2::frame::Ethernet2Header, ipv4::datagram::Ipv4Header},
    runtime::PacketBuf,
    runtime::RuntimeBuf,
};

pub use header::UdpHeader;

//==============================================================================
// Constants & Structures
//==============================================================================

///
/// UDP Packet
///
/// - TODO: write unit test for serialization
///
#[derive(Debug)]
pub struct UdpDatagram<T: RuntimeBuf> {
    /// Ethernet header.
    ethernet2_hdr: Ethernet2Header,
    /// IPv4 header.
    ipv4_hdr: Ipv4Header,
    /// UDP header.
    udp_hdr: UdpHeader,
    /// Payload
    data: T,
    /// Disable checksum?
    no_checksum: bool,
}


pub struct UdpBatchDatagram<T: RuntimeBuf> {
    /// Ethernet header.
    ethernet2_hdr: Ethernet2Header,
    /// IPv4 header.
    ipv4_hdr: Ipv4Header,
    /// UDP header.
    udp_hdr: UdpHeader,
    /// Payload
    data: Vec<T>,
    /// Disable checksum?
    no_checksum: bool,
}

//==============================================================================
// Associate Functions
//==============================================================================

// Associate functions for [UdpDatagram].
impl<T: RuntimeBuf> UdpDatagram<T> {
    /// Creates a UDP packet.
    pub fn new(
        ethernet2_hdr: Ethernet2Header,
        ipv4_hdr: Ipv4Header,
        udp_hdr: UdpHeader,
        data: T,
        no_checksum: bool,
    ) -> Self {
        Self {
            ethernet2_hdr,
            ipv4_hdr,
            udp_hdr,
            data,
            no_checksum,
        }
    }
}

// Associate functions for [UdpDatagram].
impl<T: RuntimeBuf> UdpBatchDatagram<T> {
    /// Creates a UDP packet.
    pub fn new(
        ethernet2_hdr: Ethernet2Header,
        ipv4_hdr: Ipv4Header,
        udp_hdr: UdpHeader,
        data: Vec<T>,
        no_checksum: bool,
    ) -> Self {
        Self {
            ethernet2_hdr,
            ipv4_hdr,
            udp_hdr,
            data,
            no_checksum,
        }
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Implementation of [PacketBuf] for [UdpDatagram].
impl<T: RuntimeBuf> PacketBuf<T> for UdpDatagram<T> {
    /// Computers the size of the target UDP header.
    fn header_size(&self) -> usize {
        self.ethernet2_hdr.compute_size() + self.ipv4_hdr.compute_size() + self.udp_hdr.size()
    }

    /// Computes the size of the target UDP payload.
    fn body_size(&self) -> usize {
        self.data.len()
    }

    fn has_body(&self) -> bool{
        return true;
    }

    unsafe fn get_body(&self) -> *mut T{
        return transmute::<*const T, *mut T>(&(self.data));
    }
    
    /// Serializes the header of the target UDP packet.
    fn write_header(&self, buf: &mut [u8]) {
        let mut cur_pos = 0;
        let eth_hdr_size = self.ethernet2_hdr.compute_size();
        let udp_hdr_size = self.udp_hdr.size();
        let ipv4_payload_len = udp_hdr_size + self.data.len();

        // Ethernet header.
        self.ethernet2_hdr
            .serialize(&mut buf[cur_pos..(cur_pos + eth_hdr_size)]);
        cur_pos += eth_hdr_size;

        // IPV4 header.
        let ipv4_hdr_size = self.ipv4_hdr.compute_size();
        self.ipv4_hdr.serialize(
            &mut buf[cur_pos..(cur_pos + ipv4_hdr_size)],
            ipv4_payload_len,
        );
        cur_pos += ipv4_hdr_size;

        // UDP header.
        self.udp_hdr.serialize(
            &mut buf[cur_pos..(cur_pos + udp_hdr_size)],
            &self.ipv4_hdr,
            &self.data[..],
            self.no_checksum,
        );
    }

    /// Serializes the header of the target UDP packet.
    fn write_header_index(&self, buf: &mut [u8], index:usize) {
        todo!();
    }

    unsafe fn get_batch(&self) -> *mut Vec<T>{
        todo!();
    }

    fn  if_batch(&self)  -> bool{
        return false;
    }

    /// Returns the payload of the target UDP packet.
    fn take_body(self) -> Option<T> {
        Some(self.data)
    }
}


/// Implementation of [PacketBuf] for [UdpDatagram].
impl<T: RuntimeBuf> PacketBuf<T> for UdpBatchDatagram<T> {
    /// Computers the size of the target UDP header.
    fn  if_batch(&self)  -> bool{
        return true;
    }

    fn header_size(&self) -> usize {
        self.ethernet2_hdr.compute_size() + self.ipv4_hdr.compute_size() + self.udp_hdr.size()
    }

    /// Computes the size of the target UDP payload.
    fn body_size(&self) -> usize {
        self.data.len()
    }

    fn has_body(&self) -> bool{
        return true;
    }

    unsafe fn get_batch(&self) -> *mut Vec<T>{
        // self.data.as_mut_ptr() as *mut Vec<T>
        return transmute::<*const Vec<T>, *mut Vec<T>>(&(self.data) as *const Vec<T>);
    }
    unsafe fn get_body(&self) -> *mut T{
        todo!();
    }
    
    /// Serializes the header of the target UDP packet.
    fn write_header(&self, buf: &mut [u8]) {
        todo!();
    }

    /// Serializes the header of the target UDP packet.
    fn write_header_index(&self, buf: &mut [u8], index:usize){
        let mut cur_pos = 0;
        let eth_hdr_size = self.ethernet2_hdr.compute_size();
        let udp_hdr_size = self.udp_hdr.size();
        let ipv4_payload_len = udp_hdr_size + self.data.len();

        // Ethernet header.
        self.ethernet2_hdr
            .serialize(&mut buf[cur_pos..(cur_pos + eth_hdr_size)]);
        cur_pos += eth_hdr_size;

        // IPV4 header.
        let ipv4_hdr_size = self.ipv4_hdr.compute_size();
        self.ipv4_hdr.serialize(
            &mut buf[cur_pos..(cur_pos + ipv4_hdr_size)],
            ipv4_payload_len,
        );
        cur_pos += ipv4_hdr_size;
        // UDP header.
        self.udp_hdr.serialize(
            &mut buf[cur_pos..(cur_pos + udp_hdr_size)],
            &self.ipv4_hdr,
            &(self.data[index])[..],
            self.no_checksum,
        );
    }

    /// Returns the payload of the target UDP packet.
    fn take_body(self) -> Option<T> {
        todo!();
    }
}
