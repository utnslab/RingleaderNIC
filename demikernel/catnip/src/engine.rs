// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{
    fail::Fail,
    file_table::{File, FileDescriptor, FileTable},
    operations::ResultFuture,
    protocols::{
        arp,
        ethernet2::frame::{EtherType2, Ethernet2Header},
        ipv4,
        tcp::operations::{AcceptFuture, ConnectFuture, PopFuture, PushFuture},
        udp::{UdpOperation, UdpPopFuture},
        Protocol,
    },
    runtime::Runtime,
    scheduler::Operation,
};
use std::{future::Future, net::Ipv4Addr, time::Duration};

#[cfg(test)]
use crate::protocols::ethernet2::MacAddress;
#[cfg(test)]
use std::collections::HashMap;

// TODO: Unclear why this itermediate `Engine` struct is needed.
pub struct Engine<RT: Runtime> {
    rt: RT,
    arp: arp::Peer<RT>,
    ipv4: ipv4::Peer<RT>,
    file_table: FileTable,
}

impl<RT: Runtime> Engine<RT> {
    pub fn new(rt: RT) -> Result<Self, Fail> {
        let now = rt.now();
        let file_table = FileTable::new();
        let arp = arp::Peer::new(now, rt.clone(), rt.arp_options())?;
        let ipv4 = ipv4::Peer::new(rt.clone(), arp.clone(), file_table.clone());
        Ok(Engine {
            rt,
            arp,
            ipv4,
            file_table,
        })
    }

    pub fn rt(&self) -> &RT {
        &self.rt
    }

    /// New incoming data has arrived. Route it to the correct parse out the Ethernet header and
    /// allow the correct protocol to handle it. The underlying protocol will futher parse the data
    /// and inform the correct task that its data has arrived.
    pub fn receive(&mut self, bytes: RT::Buf) -> Result<(), Fail> {
        let (header, payload) = Ethernet2Header::parse(bytes)?;
        debug!("Engine received {:?}", header);
        if self.rt.local_link_addr() != header.dst_addr && !header.dst_addr.is_broadcast() {
            return Err(Fail::Ignored {
                details: "Physical dst_addr mismatch",
            });
        }
        match header.ether_type {
            EtherType2::Arp => self.arp.receive(payload),
            EtherType2::Ipv4 => self.ipv4.receive(payload),
        }
    }

    pub fn ping(
        &mut self,
        dest_ipv4_addr: Ipv4Addr,
        timeout: Option<Duration>,
    ) -> impl Future<Output = Result<Duration, Fail>> {
        self.ipv4.ping(dest_ipv4_addr, timeout)
    }

    pub fn socket(&mut self, protocol: Protocol) -> FileDescriptor {
        match protocol {
            Protocol::Tcp => self.ipv4.tcp.socket(),
            Protocol::Udp => self.ipv4.udp.socket().unwrap(),
        }
    }

    pub fn connect(
        &mut self,
        fd: FileDescriptor,
        remote_endpoint: ipv4::Endpoint,
    ) -> Result<Operation<RT>, Fail> {
        match self.file_table.get(fd) {
            Some(File::TcpSocket) => {
                Ok(Operation::from(self.ipv4.tcp.connect(fd, remote_endpoint)))
            }
            Some(File::UdpSocket) => {
                let udp_op =
                    UdpOperation::<RT>::Connect(fd, self.ipv4.udp.connect(fd, remote_endpoint));
                Ok(Operation::Udp(udp_op))
            }
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn bind(&mut self, fd: FileDescriptor, endpoint: ipv4::Endpoint) -> Result<(), Fail> {
        match self.file_table.get(fd) {
            Some(File::TcpSocket) => self.ipv4.tcp.bind(fd, endpoint),
            Some(File::UdpSocket) => self.ipv4.udp.bind(fd, endpoint),
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn accept(&mut self, fd: FileDescriptor) -> Result<Operation<RT>, Fail> {
        match self.file_table.get(fd) {
            Some(File::TcpSocket) => Ok(Operation::from(self.ipv4.tcp.accept(fd))),
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn listen(&mut self, fd: FileDescriptor, backlog: usize) -> Result<(), Fail> {
        match self.file_table.get(fd) {
            Some(File::TcpSocket) => self.ipv4.tcp.listen(fd, backlog),
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn push(&mut self, fd: FileDescriptor, buf: RT::Buf) -> Result<Operation<RT>, Fail> {
        match self.file_table.get(fd) {
            Some(File::TcpSocket) => Ok(Operation::from(self.ipv4.tcp.push(fd, buf))),
            Some(File::UdpSocket) => {
                let udp_op = UdpOperation::Push(fd, self.ipv4.udp.push(fd, buf));
                Ok(Operation::Udp(udp_op))
            }
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn dyield( &mut self,
        fd: FileDescriptor,
    )-> Result<Operation<RT>, Fail> {
        match self.file_table.get(fd) {
            Some(File::UdpSocket) => {
                let udp_op = UdpOperation::Dyield(fd, Ok(()));
                Ok(Operation::Udp(udp_op))
            }
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn pushto(
        &mut self,
        fd: FileDescriptor,
        buf: RT::Buf,
        to: ipv4::Endpoint,
    ) -> Result<Operation<RT>, Fail> {
        match self.file_table.get(fd) {
            Some(File::UdpSocket) => {
                let udp_op = UdpOperation::Push(fd, self.ipv4.udp.pushto(fd, buf, to));
                Ok(Operation::Udp(udp_op))
            }
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn batchpushto(
        &mut self,
        fd: FileDescriptor,
        batch: Vec<(ipv4::Endpoint, RT::Buf)>,
    ) -> Result<Operation<RT>, Fail> {
        match self.file_table.get(fd) {
            Some(File::UdpSocket) => {
                let udp_op = UdpOperation::Push(fd, self.ipv4.udp.batchpushto(fd,batch));
                Ok(Operation::Udp(udp_op))
            }
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }


    pub fn udp_push(&mut self, fd: FileDescriptor, buf: RT::Buf) -> Result<(), Fail> {
        self.ipv4.udp.push(fd, buf)
    }

    pub fn udp_pushto(
        &self,
        fd: FileDescriptor,
        buf: RT::Buf,
        to: ipv4::Endpoint,
    ) -> Result<(), Fail> {
        self.ipv4.udp.pushto(fd, buf, to)
    }

    pub fn udp_pop(&mut self, fd: FileDescriptor) -> UdpPopFuture<RT> {
        self.ipv4.udp.pop(fd)
    }

    pub fn pop(&mut self, fd: FileDescriptor) -> Result<Operation<RT>, Fail> {
        match self.file_table.get(fd) {
            Some(File::TcpSocket) => Ok(Operation::from(self.ipv4.tcp.pop(fd))),
            Some(File::UdpSocket) => {
                let udp_op = UdpOperation::Pop(ResultFuture::new(self.ipv4.udp.pop(fd)));
                Ok(Operation::Udp(udp_op))
            }
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn popbatch(&mut self, fd: FileDescriptor) -> Result<Operation<RT>, Fail> {
        match self.file_table.get(fd) {
            Some(File::UdpSocket) => {
                let udp_op = UdpOperation::PopBatch(ResultFuture::new(self.ipv4.udp.popbatch(fd)));
                Ok(Operation::Udp(udp_op))
            }
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn close(&mut self, fd: FileDescriptor) -> Result<(), Fail> {
        match self.file_table.get(fd) {
            Some(File::TcpSocket) => self.ipv4.tcp.close(fd),
            Some(File::UdpSocket) => self.ipv4.udp.close(fd),
            _ => Err(Fail::BadFileDescriptor {}),
        }
    }

    pub fn udp_socket(&mut self) -> Result<FileDescriptor, Fail> {
        self.ipv4.udp.socket()
    }

    pub fn udp_bind(
        &mut self,
        socket_fd: FileDescriptor,
        endpoint: ipv4::Endpoint,
    ) -> Result<(), Fail> {
        self.ipv4.udp.bind(socket_fd, endpoint)
    }

    pub fn tcp_socket(&mut self) -> FileDescriptor {
        self.ipv4.tcp.socket()
    }

    pub fn tcp_connect(
        &mut self,
        socket_fd: FileDescriptor,
        remote_endpoint: ipv4::Endpoint,
    ) -> ConnectFuture<RT> {
        self.ipv4.tcp.connect(socket_fd, remote_endpoint)
    }

    pub fn tcp_bind(
        &mut self,
        socket_fd: FileDescriptor,
        endpoint: ipv4::Endpoint,
    ) -> Result<(), Fail> {
        self.ipv4.tcp.bind(socket_fd, endpoint)
    }

    pub fn tcp_accept(&mut self, handle: FileDescriptor) -> AcceptFuture<RT> {
        self.ipv4.tcp.accept(handle)
    }

    pub fn tcp_push(&mut self, socket_fd: FileDescriptor, buf: RT::Buf) -> PushFuture<RT> {
        self.ipv4.tcp.push(socket_fd, buf)
    }

    pub fn tcp_pop(&mut self, socket_fd: FileDescriptor) -> PopFuture<RT> {
        self.ipv4.tcp.pop(socket_fd)
    }

    pub fn tcp_close(&mut self, socket_fd: FileDescriptor) -> Result<(), Fail> {
        self.ipv4.tcp.close(socket_fd)
    }

    pub fn tcp_listen(&mut self, socket_fd: FileDescriptor, backlog: usize) -> Result<(), Fail> {
        self.ipv4.tcp.listen(socket_fd, backlog)
    }

    #[cfg(test)]
    pub fn arp_query(&self, ipv4_addr: Ipv4Addr) -> impl Future<Output = Result<MacAddress, Fail>> {
        self.arp.query(ipv4_addr)
    }

    #[cfg(test)]
    pub fn tcp_mss(&self, handle: FileDescriptor) -> Result<usize, Fail> {
        self.ipv4.tcp_mss(handle)
    }

    #[cfg(test)]
    pub fn tcp_rto(&self, handle: FileDescriptor) -> Result<Duration, Fail> {
        self.ipv4.tcp_rto(handle)
    }

    #[cfg(test)]
    pub fn export_arp_cache(&self) -> HashMap<Ipv4Addr, MacAddress> {
        self.arp.export_cache()
    }
}
