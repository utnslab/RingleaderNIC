// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
use crate::{
    interop::dmtr_sgarray_t,
    protocols::{arp, ethernet2::MacAddress, tcp, udp},
    scheduler::{Operation, Scheduler, SchedulerHandle},
};
use arrayvec::ArrayVec;
use rand::distributions::{Distribution, Standard};
use std::{collections::HashMap};
use std::{
    fmt::Debug,
    future::Future,
    net::Ipv4Addr,
    ops::Deref,
    time::{Duration, Instant},
};

pub const RECEIVE_BATCH_SIZE: usize = 16;

pub trait RuntimeBuf: Clone + Debug + Deref<Target = [u8]> + Sized + Unpin {
    fn empty() -> Self;

    fn from_slice(bytes: &[u8]) -> Self;

    /// Remove `num_bytes` from the beginning of the buffer.
    fn adjust(&mut self, num_bytes: usize);
    /// Remove `num_bytes` from the end of the buffer;
    fn trim(&mut self, num_bytes: usize);

    unsafe fn slice_mut(&mut self) -> &mut [u8] {
        panic!("not implemented");
    }
}
pub trait PacketBuf<T>: Sized {
    fn if_batch(&self) -> bool;
    
    fn header_size(&self) -> usize;
    fn write_header(&self, buf: &mut [u8]);
    fn write_header_index(&self, buf: &mut [u8], index:usize);
    fn body_size(&self) -> usize;
    fn take_body(self) -> Option<T>;

    fn has_body(&self) -> bool;
    unsafe fn get_body(& self) -> *mut T;
    unsafe fn get_batch(&self) -> *mut Vec<T>;
}

/// Common interface that tranport layers should implement? E.g. DPDK and RDMA.
pub trait Runtime: Clone + Unpin + 'static {
    type Buf: RuntimeBuf;
    type WaitFuture: Future<Output = ()>;

    #[allow(clippy::wrong_self_convention)]
    fn into_sgarray(&self, buf: Self::Buf) -> dmtr_sgarray_t;
    fn alloc_sgarray(&self, size: usize) -> dmtr_sgarray_t;
    fn free_sgarray(&self, sga: dmtr_sgarray_t);
    fn clone_sgarray(&self, sga: &dmtr_sgarray_t) -> Self::Buf;

    fn advance_clock(&self, now: Instant);
    fn transmit(&self, pkt: impl PacketBuf<Self::Buf>);
    fn transmit_batch(&self, pkt:  Vec<impl PacketBuf<Self::Buf>>);
    fn receive(&self) -> (ArrayVec<(u16, u16),RECEIVE_BATCH_SIZE>, ArrayVec<Self::Buf, RECEIVE_BATCH_SIZE>);

    fn local_link_addr(&self) -> MacAddress;
    fn local_ipv4_addr(&self) -> Ipv4Addr;
    fn arp_options(&self) -> arp::Options;
    fn tcp_options(&self) -> tcp::Options<Self>;
    fn udp_options(&self) -> udp::Options;

    fn wait(&self, duration: Duration) -> Self::WaitFuture;
    fn wait_until(&self, when: Instant) -> Self::WaitFuture;
    fn now(&self) -> Instant;

    fn rng_gen<T>(&self) -> T
    where
        Standard: Distribution<T>;
    fn rng_shuffle<T>(&self, slice: &mut [T]);

    fn spawn<F: Future<Output = ()> + 'static>(&self, future: F) -> SchedulerHandle;
    fn scheduler(&self) -> &Scheduler<Operation<Self>>;
}
