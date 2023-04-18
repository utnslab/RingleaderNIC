// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{
    fail::Fail,
    protocols::{
        ip,
        ipv4::datagram::{Ipv4Header, Ipv4Protocol2},
    },
    runtime::RuntimeBuf,
};

use byteorder::{ByteOrder, NetworkEndian};

use std::convert::{TryFrom, TryInto};

//==============================================================================
// Constants & Structures
//==============================================================================

/// Size of a UDP header (in bytes).
const UDP_HEADER_SIZE: usize = 8;

///
/// Header for UDP Packets
///
/// - NOTE: length and checksum are omitted from this structure, because they
/// are computed on-the-fly when parsing/serializing UDP headers.
///
/// - TODO: write unit test for checksum computation
/// - TODO: write unit test for parsing/serializing
///
#[derive(Debug)]
pub struct UdpHeader {
    /// Port used on sender side (optional).
    src_port: Option<ip::Port>,
    /// Port used receiver side.
    dst_port: ip::Port,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [UdpHeader].
impl UdpHeader {
    /// Creates a UDP header.
    pub fn new(src_port: Option<ip::Port>, dst_port: ip::Port) -> Self {
        Self { src_port, dst_port }
    }

    /// Returns the source port stored in the target UDP header.
    pub fn src_port(&self) -> Option<ip::Port> {
        self.src_port
    }

    /// Returns the destination port stored in the target UDP header.
    pub fn dest_port(&self) -> ip::Port {
        self.dst_port
    }

    /// Returns the size of the target UDP header (in bytes).
    pub fn size(&self) -> usize {
        UDP_HEADER_SIZE
    }

    /// Parses a buffer into an UDP header.
    pub fn parse<T: RuntimeBuf>(
        ipv4_header: &Ipv4Header,
        mut buf: T,
        no_chsecksum: bool,
    ) -> Result<(Self, T), Fail> {
        // Malformed header.
        if buf.len() < UDP_HEADER_SIZE {
            return Err(Fail::Malformed {
                details: "UDP segment too small",
            });
        }

        // Deserialize buffer.
        let hdr_buf = &buf[..UDP_HEADER_SIZE];
        let src_port = ip::Port::try_from(NetworkEndian::read_u16(&hdr_buf[0..2])).ok();
        let dst_port = ip::Port::try_from(NetworkEndian::read_u16(&hdr_buf[2..4]))?;
        let length = NetworkEndian::read_u16(&hdr_buf[4..6]) as usize;
        if length != buf.len() {
            return Err(Fail::Malformed {
                details: "UDP length mismatch",
            });
        }

        // Verify payload.
        if !no_chsecksum {
            let payload_buf = &buf[UDP_HEADER_SIZE..];
            let checksum = NetworkEndian::read_u16(&hdr_buf[6..8]);
            if checksum != 0 && checksum != Self::checksum(&ipv4_header, hdr_buf, payload_buf) {
                return Err(Fail::Malformed {
                    details: "UDP checksum mismatch",
                });
            }
        }

        let header = Self::new(src_port, dst_port);
        buf.adjust(UDP_HEADER_SIZE);
        Ok((header, buf))
    }

    /// Serializes the target UDP header.
    pub fn serialize(
        &self,
        buf: &mut [u8],
        ipv4_hdr: &Ipv4Header,
        data: &[u8],
        no_chsecksum: bool,
    ) {
        let fixed_buf: &mut [u8; UDP_HEADER_SIZE] =
            (&mut buf[..UDP_HEADER_SIZE]).try_into().unwrap();

        // Source port.
        NetworkEndian::write_u16(
            &mut fixed_buf[0..2],
            self.src_port.map(|p| p.into()).unwrap_or(0),
        );

        // Destination port.
        NetworkEndian::write_u16(&mut fixed_buf[2..4], self.dst_port.into());

        // Payload length.
        NetworkEndian::write_u16(&mut fixed_buf[4..6], (UDP_HEADER_SIZE + data.len()) as u16);

        // Checksum.
        let checksum = if no_chsecksum {
            0
        } else {
            Self::checksum(ipv4_hdr, &fixed_buf[..], data)
        };
        NetworkEndian::write_u16(&mut fixed_buf[6..8], checksum);
    }

    ///
    /// Computes the checksum of an UDP packet.
    ///
    /// This is the 16-bit one's complement of the one's complement sum of a
    /// pseudo header of information from the IP header, the UDP header, and the
    /// data,  padded  with zero octets at the end (if  necessary)  to  make  a
    /// multiple of two octets.
    ///
    fn checksum(ipv4_header: &Ipv4Header, header: &[u8], data: &[u8]) -> u16 {
        let mut state = 0xffffu32;

        // Source address (4 bytes)
        let src_octets = ipv4_header.src_addr.octets();
        state += NetworkEndian::read_u16(&src_octets[0..2]) as u32;
        state += NetworkEndian::read_u16(&src_octets[2..4]) as u32;

        // Destination address (4 bytes)
        let dst_octets = ipv4_header.dst_addr.octets();
        state += NetworkEndian::read_u16(&dst_octets[0..2]) as u32;
        state += NetworkEndian::read_u16(&dst_octets[2..4]) as u32;

        // Padding zeros (1 byte) and UDP protocol number (1 byte)
        state += NetworkEndian::read_u16(&[0, Ipv4Protocol2::Udp as u8]) as u32;

        // UDP segment length (2 bytes)
        state += (header.len() + data.len()) as u32;

        // Switch to UDP header.
        let fixed_header: &[u8; UDP_HEADER_SIZE] = header.try_into().unwrap();

        // Source port (2 bytes)
        state += NetworkEndian::read_u16(&fixed_header[0..2]) as u32;

        // Destination port (2 bytes)
        state += NetworkEndian::read_u16(&fixed_header[2..4]) as u32;

        // Payload Length (2 bytes)
        state += NetworkEndian::read_u16(&fixed_header[4..6]) as u32;

        // Checksum (2 bytes, all zeros)
        state += 0;

        // Payload.
        let mut chunks_iter = data.chunks_exact(2);
        while let Some(chunk) = chunks_iter.next() {
            state += NetworkEndian::read_u16(chunk) as u32;
        }
        // Pad with zeros with payload has an odd number of bytes.
        if let Some(&b) = chunks_iter.remainder().get(0) {
            state += NetworkEndian::read_u16(&[b, 0]) as u32;
        }

        // NOTE: We don't need to subtract out 0xFFFF as we accumulate the sum.
        // Since we use a u32 for intermediate state, we would need 2^16
        // additions to overflow. This is well beyond the reach of the largest
        // jumbo frames. The upshot is that the compiler can then optimize this
        // final loop into a single branch-free code.
        while state > 0xFFFF {
            state -= 0xFFFF;
        }
        !state as u16
    }
}
