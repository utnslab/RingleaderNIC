// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use arrayvec::ArrayVec;

use catnip::{
    collections::bytes::{Bytes, BytesMut},
    interop::dmtr_sgarray_t,
    interop::dmtr_sgaseg_t,
    protocols::ethernet2::MacAddress,
    protocols::{arp, tcp, udp},
    runtime::Runtime,
    runtime::{PacketBuf, RECEIVE_BATCH_SIZE},
    scheduler::{Operation, Scheduler, SchedulerHandle},
    timer::{Timer, TimerRc},
};

use crossbeam_channel::{self};

use futures::FutureExt;

use rand::{
    distributions::{Distribution, Standard},
    rngs::SmallRng,
    seq::SliceRandom,
    Rng, SeedableRng,
};

use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    mem,
    net::Ipv4Addr,
    ptr,
    rc::Rc,
    slice,
    time::{Duration, Instant},
};

//==============================================================================
// Constants & Structures
//==============================================================================

#[derive(Clone)]
pub struct DummyRuntime {
    inner: Rc<RefCell<Inner>>,
    scheduler: Scheduler<Operation<DummyRuntime>>,
}

struct Inner {
    timer: TimerRc,
    rng: SmallRng,
    incoming: crossbeam_channel::Receiver<Bytes>,
    outgoing: crossbeam_channel::Sender<Bytes>,

    link_addr: MacAddress,
    ipv4_addr: Ipv4Addr,
    tcp_options: tcp::Options<DummyRuntime>,
    arp_options: arp::Options,
}

//==============================================================================
// Associate Functions
//==============================================================================

impl DummyRuntime {
    pub fn new(
        now: Instant,
        link_addr: MacAddress,
        ipv4_addr: Ipv4Addr,
        incoming: crossbeam_channel::Receiver<Bytes>,
        outgoing: crossbeam_channel::Sender<Bytes>,
        arp: HashMap<Ipv4Addr, MacAddress>,
    ) -> Self {
        let mut arp_options = arp::Options::default();
        arp_options.retry_count = 2;
        arp_options.cache_ttl = Duration::from_secs(600);
        arp_options.request_timeout = Duration::from_secs(1);
        arp_options.initial_values = arp;

        let inner = Inner {
            timer: TimerRc(Rc::new(Timer::new(now))),
            rng: SmallRng::from_seed([0; 32]),
            incoming,
            outgoing,
            link_addr,
            ipv4_addr,
            tcp_options: tcp::Options::default(),
            arp_options,
        };
        Self {
            inner: Rc::new(RefCell::new(inner)),
            scheduler: Scheduler::new(),
        }
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

impl Runtime for DummyRuntime {
    type WaitFuture = catnip::timer::WaitFuture<TimerRc>;
    type Buf = Bytes;

    fn into_sgarray(&self, buf: Bytes) -> dmtr_sgarray_t {
        let buf_copy: Box<[u8]> = (&buf[..]).into();
        let ptr = Box::into_raw(buf_copy);
        let sgaseg = dmtr_sgaseg_t {
            sgaseg_buf: ptr as *mut _,
            sgaseg_len: buf.len() as u32,
        };
        dmtr_sgarray_t {
            sga_buf: ptr::null_mut(),
            sga_numsegs: 1,
            sga_segs: [sgaseg],
            sga_addr: unsafe { mem::zeroed() },
        }
    }

    fn alloc_sgarray(&self, size: usize) -> dmtr_sgarray_t {
        let allocation: Box<[u8]> = unsafe { Box::new_uninit_slice(size).assume_init() };
        let ptr = Box::into_raw(allocation);
        let sgaseg = dmtr_sgaseg_t {
            sgaseg_buf: ptr as *mut _,
            sgaseg_len: size as u32,
        };
        dmtr_sgarray_t {
            sga_buf: ptr::null_mut(),
            sga_numsegs: 1,
            sga_segs: [sgaseg],
            sga_addr: unsafe { mem::zeroed() },
        }
    }

    fn free_sgarray(&self, sga: dmtr_sgarray_t) {
        assert_eq!(sga.sga_numsegs, 1);
        for i in 0..sga.sga_numsegs as usize {
            let seg = &sga.sga_segs[i];
            let allocation: Box<[u8]> = unsafe {
                Box::from_raw(slice::from_raw_parts_mut(
                    seg.sgaseg_buf as *mut _,
                    seg.sgaseg_len as usize,
                ))
            };
            drop(allocation);
        }
    }

    fn clone_sgarray(&self, sga: &dmtr_sgarray_t) -> Bytes {
        let mut len = 0;
        for i in 0..sga.sga_numsegs as usize {
            len += sga.sga_segs[i].sgaseg_len;
        }
        let mut buf = BytesMut::zeroed(len as usize).unwrap();
        let mut pos = 0;
        for i in 0..sga.sga_numsegs as usize {
            let seg = &sga.sga_segs[i];
            let seg_slice = unsafe {
                slice::from_raw_parts(seg.sgaseg_buf as *mut u8, seg.sgaseg_len as usize)
            };
            buf[pos..(pos + seg_slice.len())].copy_from_slice(seg_slice);
            pos += seg_slice.len();
        }
        buf.freeze()
    }

    fn transmit(&self, pkt: impl PacketBuf<Bytes>) {
        let header_size = pkt.header_size();
        let body_size = pkt.body_size();

        let mut buf = BytesMut::zeroed(header_size + body_size).unwrap();
        pkt.write_header(&mut buf[..header_size]);
        if let Some(body) = pkt.take_body() {
            buf[header_size..].copy_from_slice(&body[..]);
        }
        self.inner
            .borrow_mut()
            .outgoing
            .try_send(buf.freeze())
            .unwrap();
    }

    fn receive(&self) -> ArrayVec<Bytes, RECEIVE_BATCH_SIZE> {
        let mut out = ArrayVec::new();
        if let Some(buf) = self.inner.borrow_mut().incoming.try_recv().ok() {
            out.push(buf);
        }
        out
    }

    fn scheduler(&self) -> &Scheduler<Operation<Self>> {
        &self.scheduler
    }

    fn local_link_addr(&self) -> MacAddress {
        self.inner.borrow().link_addr.clone()
    }

    fn local_ipv4_addr(&self) -> Ipv4Addr {
        self.inner.borrow().ipv4_addr.clone()
    }

    fn tcp_options(&self) -> tcp::Options<Self> {
        self.inner.borrow().tcp_options.clone()
    }

    fn udp_options(&self) -> udp::Options {
        udp::Options::default()
    }

    fn arp_options(&self) -> arp::Options {
        self.inner.borrow().arp_options.clone()
    }

    fn advance_clock(&self, now: Instant) {
        self.inner.borrow_mut().timer.0.advance_clock(now);
    }

    fn wait(&self, duration: Duration) -> Self::WaitFuture {
        let inner = self.inner.borrow_mut();
        let now = inner.timer.0.now();
        inner
            .timer
            .0
            .wait_until(inner.timer.clone(), now + duration)
    }

    fn wait_until(&self, when: Instant) -> Self::WaitFuture {
        let inner = self.inner.borrow_mut();
        inner.timer.0.wait_until(inner.timer.clone(), when)
    }

    fn now(&self) -> Instant {
        self.inner.borrow().timer.0.now()
    }

    fn rng_gen<T>(&self) -> T
    where
        Standard: Distribution<T>,
    {
        let mut inner = self.inner.borrow_mut();
        inner.rng.gen()
    }

    fn rng_shuffle<T>(&self, slice: &mut [T]) {
        let mut inner = self.inner.borrow_mut();
        slice.shuffle(&mut inner.rng);
    }

    fn spawn<F: Future<Output = ()> + 'static>(&self, future: F) -> SchedulerHandle {
        self.scheduler
            .insert(Operation::Background(future.boxed_local()))
    }
}
