// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

//! LibOS defines the PDPIX (portable data plane interface) abstraction. PDPIX centers around
//! the IO Queue abstraction, thus providing a standard interface for different kernel bypass
//! mechanisms.
//#[macro_use]
//extern crate lazy_static;
use crate::{
    engine::Engine,
    fail::Fail,
    file_table::FileDescriptor,
    interop::{dmtr_qresult_t, dmtr_sgarray_t},
    operations::OperationResult,
    protocols::ipv4::Endpoint,
    protocols::Protocol,
    runtime::{Runtime,RECEIVE_BATCH_SIZE},
    scheduler::{Operation, SchedulerHandle},
};
use arrayvec::ArrayVec;
use libc::c_int;
use must_let::must_let;
use std::time::Instant;
use std::time::{UNIX_EPOCH, SystemTime};
use std::{collections::HashMap};
use std::sync::Mutex;
#[cfg(feature = "profiler")]
use perftools::timer;
use crossbeam_channel::{bounded, Sender, Receiver};
use std::sync::Arc;
use bit_array::BitArray;
use typenum::U64;

const TIMER_RESOLUTION: usize = 64;
const MAX_RECV_ITERS: usize = 2;
const MAX_CHANNEL_NUM: usize = 64;
const MAX_APP_NUM: usize = 16;
/// Queue Token for our IO Queue abstraction. Analogous to a file descriptor in POSIX.
pub type QToken = u64;

pub struct LibOS<RT: Runtime> {
    engine: Engine<RT>,
    rt: RT,
    ts_iters: usize,
    msg_send_channels: ArrayVec<Sender<(u16, u16, u8)>, MAX_CHANNEL_NUM>,
    msg_recv_channels: Receiver<(u16, u16, u8)>,
    total_usable_cores: u16,
    usable_core_mask: u64,
    // bitmask: Arc<ArrayVec<[u8; MAX_CHANNEL_NUM], MAX_APP_NUM>>,
}

lazy_static! {
    static ref app_to_core_bitmasks: Mutex<HashMap<u16, u64>> = {
        let mut m = HashMap::new();
        Mutex::new(m)
    };

    static ref msg_recv_channels: Mutex<(ArrayVec<Sender<(u16, u16, u8)>, MAX_CHANNEL_NUM>, ArrayVec<Receiver<(u16, u16, u8)>, MAX_CHANNEL_NUM>)> = {
        let mut senders = ArrayVec::new();
        let mut receivers = ArrayVec::new();

        for i in 0..MAX_CHANNEL_NUM {
            let (s, r) = bounded(2);    
            senders.push(s);
            receivers.push(r);
        }
        Mutex::new((senders,receivers))
    };
}

//static mut core_to_app: HashMap<u16, ArrayVec<u16,RECEIVE_BATCH_SIZE> > = HashMap::new();
//let pub hashlock = Arc::new(Mutex::new(appid_to_port));
impl<RT: Runtime> LibOS<RT> {
    pub fn new(rt: RT, core_id: usize, core_count: u16) -> Result<Self, Fail> {
        assert!(core_count as usize <= MAX_CHANNEL_NUM);
        let engine = Engine::new(rt.clone())?;
        let mut sender = ArrayVec::new();
  
        let global_channel_list = msg_recv_channels.lock().unwrap();
        let global_sender_list = &global_channel_list.0;
        let global_receiver_list = &global_channel_list.1;
        
        for i in global_sender_list{
            sender.push(i.clone());
        }
        let receiver = global_receiver_list.get(core_id).unwrap().clone();
        Ok(Self {
            engine,
            rt,
            ts_iters: 0,
            msg_send_channels: sender,
            msg_recv_channels: receiver,
            total_usable_cores: core_count,
            usable_core_mask: 0xffffffffffffffff << (core_count),
            // bitmask: app_to_core_bitmasks.clone(),
        })
    }

    pub fn rt(&self) -> &RT {
        &self.rt
    }

    ///
    /// **Brief**
    ///
    /// Creates an endpoint for communication and returns a file descriptor that
    /// refers to that endpoint. The file descriptor returned by a successful
    /// call will be the lowest numbered file descriptor not currently open for
    /// the process.
    ///
    /// The domain argument specifies a communication domain; this selects the
    /// protocol family which will be used for communication. These families are
    /// defined in the libc crate. Currently, the following families are supported:
    ///
    /// - AF_INET Internet Protocol Version 4 (IPv4)
    ///
    /// **Return Vale**
    ///
    /// Upon successful completion, a file descriptor for the newly created
    /// socket is returned. Upon failure, `Fail` is returned instead.
    ///
    pub fn socket(
        &mut self,
        domain: c_int,
        socket_type: c_int,
        _protocol: c_int,
    ) -> Result<FileDescriptor, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::socket");
        trace!(
            "socket(): domain={:?} type={:?} protocol={:?}",
            domain,
            socket_type,
            _protocol
        );
        if domain != libc::AF_INET {
            return Err(Fail::AddressFamilySupport {});
        }
        let engine_protocol = match socket_type {
            libc::SOCK_STREAM => Protocol::Tcp,
            libc::SOCK_DGRAM => Protocol::Udp,
            _ => return Err(Fail::SocketTypeSupport {}),
        };
        Ok(self.engine.socket(engine_protocol))
    }

    ///
    /// **Brief**
    ///
    /// Binds the socket referred to by `fd` to the local endpoint specified by
    /// `local`.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, `Ok(())` is returned. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn bind(&mut self, fd: FileDescriptor, local: Endpoint) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::bind");
        trace!("bind(): fd={:?} local={:?}", fd, local);
        self.engine.bind(fd, local)
    }

    ///
    /// **Brief**
    ///
    /// Marks the socket referred to by `fd` as a socket that will be used to
    /// accept incoming connection requests using [accept](Self::accept). The `fd` should
    /// refer to a socket of type `SOCK_STREAM`. The `backlog` argument defines
    /// the maximum length to which the queue of pending connections for `fd`
    /// may grow. If a connection request arrives when the queue is full, the
    /// client may receive an error with an indication that the connection was
    /// refused.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, `Ok(())` is returned. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn listen(&mut self, fd: FileDescriptor, backlog: usize) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::listen");
        trace!("listen(): fd={:?} backlog={:?}", fd, backlog);
        if backlog == 0 {
            return Err(Fail::Invalid {
                details: "backlog length",
            });
        }
        self.engine.listen(fd, backlog)
    }

    ///
    /// **Brief**
    ///
    /// Accepts an incoming connection request on the queue of pending
    /// connections for the listening socket referred to by `fd`.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, a queue token is returned. This token can be
    /// used to wait for a connection request to arrive. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn accept(&mut self, fd: FileDescriptor) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::accept");
        trace!("accept(): {:?}", fd);
        match self.engine.accept(fd) {
            Ok(future) => Ok(self.rt.scheduler().insert(future).into_raw()),
            Err(fail) => Err(fail),
        }
    }

    ///
    /// **Brief**
    ///
    /// Connects the socket referred to by `fd` to the remote endpoint specified by `remote`.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, a queue token is returned. This token can be
    /// used to push and pop data to/from the queue that connects the local and
    /// remote endpoints. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn connect(&mut self, fd: FileDescriptor, remote: Endpoint) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::connect");
        trace!("connect(): fd={:?} remote={:?}", fd, remote);
        let future = self.engine.connect(fd, remote)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    ///
    /// **Brief**
    ///
    /// Closes a connection referred to by `fd`.
    ///
    /// **Return Value**
    ///
    /// Upon successful completion, `Ok(())` is returned. Upon failure, `Fail` is
    /// returned instead.
    ///
    pub fn close(&mut self, fd: FileDescriptor) -> Result<(), Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::close");
        trace!("close(): fd={:?}", fd);
        self.engine.close(fd)
    }

    /// Create a push request for Demikernel to asynchronously write data from `sga` to the
    /// IO connection represented by `fd`. This operation returns immediately with a `QToken`.
    /// The data has been written when [`wait`ing](Self::wait) on the QToken returns.
    pub fn push(&mut self, fd: FileDescriptor, sga: &dmtr_sgarray_t) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::push");
        trace!("push(): fd={:?}", fd);
        let buf = self.rt.clone_sgarray(sga);
        if buf.len() == 0 {
            return Err(Fail::Invalid {
                details: "zero-length buffer",
            });
        }
        let future = self.engine.push(fd, buf)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    /// Similar to [push](Self::push) but uses a [Runtime]-specific buffer instead of the
    /// [dmtr_sgarray_t].
    pub fn push2(&mut self, fd: FileDescriptor, buf: RT::Buf) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::push2");
        trace!("push2(): fd={:?}", fd);
        if buf.len() == 0 {
            return Err(Fail::Invalid {
                details: "zero-length buffer",
            });
        }
        let future = self.engine.push(fd, buf)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    pub fn pushto(
        &mut self,
        fd: FileDescriptor,
        sga: &dmtr_sgarray_t,
        to: Endpoint,
    ) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::pushto");
        let buf = self.rt.clone_sgarray(sga);
        if buf.len() == 0 {
            return Err(Fail::Invalid {
                details: "zero-length buffer",
            });
        }
        let future = self.engine.pushto(fd, buf, to)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    pub fn pushto2(
        &mut self,
        fd: FileDescriptor,
        buf: RT::Buf,
        to: Endpoint,
    ) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::pushto2");
        if buf.len() == 0 {
            return Err(Fail::Invalid {
                details: "zero-length buffer",
            });
        }
        let future = self.engine.pushto(fd, buf, to)?;
        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    pub fn dyield(
        &mut self,
        fd: FileDescriptor,
        prio: usize,
    ) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::pushto2");
        let future = self.engine.dyield(fd)?;
        Ok(self.rt.scheduler().insert_page(future, prio).into_raw())
    }


    pub fn directpushto2(
        &mut self,
        fd: FileDescriptor,
        buf: RT::Buf,
        to: Endpoint,
    ) -> Result<Operation<RT>, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::pushto2");
        if buf.len() == 0 {
            return Err(Fail::Invalid {
                details: "zero-length buffer",
            });
        }
        self.engine.pushto(fd, buf, to)
    }

    pub fn directbatchpushto2(
        &mut self,
        fd: FileDescriptor,
        batch: Vec<(Endpoint, RT::Buf)>,
    ) -> Result<Operation<RT>, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::pushto2");
        if batch.len() == 0 {
            return Err(Fail::Invalid {
                details: "zero-length buffer",
            });
        }
        self.engine.batchpushto(fd, batch)
    }

    ///
    /// **Brief**
    ///
    /// Invalidates the queue token referred to by `qt`. Any operations on this
    /// operations will fail.
    ///
    pub fn drop_qtoken(&mut self, qt: QToken) {
        #[cfg(feature = "profiler")]
        timer!("catnip::drop_qtoken");
        drop(self.rt.scheduler().from_raw_handle(qt).unwrap());
    }

    /// Create a pop request to write data from IO connection represented by `fd` into a buffer
    /// allocated by the application.
    pub fn pop(&mut self, fd: FileDescriptor) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::pop");

        trace!("pop(): fd={:?}", fd);

        let future = self.engine.pop(fd)?;

        Ok(self.rt.scheduler().insert(future).into_raw())
    }

    pub fn popprio(&mut self, fd: FileDescriptor, prio: usize) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::pop");

        trace!("pop(): fd={:?}", fd);

        let future = self.engine.pop(fd)?;

        Ok(self.rt.scheduler().insert_page(future, prio).into_raw())
    }


    pub fn popbatch(&mut self, fd: FileDescriptor, prio: usize) -> Result<QToken, Fail> {
        #[cfg(feature = "profiler")]
        timer!("catnip::pop");

        trace!("pop(): fd={:?}", fd);

        let future = self.engine.popbatch(fd)?;

        Ok(self.rt.scheduler().insert_page(future, prio).into_raw())
    }

    // If this returns a result, `qt` is no longer valid.
    pub fn poll(&mut self, qt: QToken) -> Option<dmtr_qresult_t> {
        #[cfg(feature = "profiler")]
        timer!("catnip::poll");
        trace!("poll(): qt={:?}", qt);
        self.poll_bg_work();
        let handle = match self.rt.scheduler().from_raw_handle(qt) {
            None => {
                panic!("Invalid handle {}", qt);
            }
            Some(h) => h,
        };
        if !handle.has_completed() {
            handle.into_raw();
            return None;
        }
        let (qd, r) = self.take_operation(handle);
        Some(dmtr_qresult_t::pack(&self.rt, r, qd, qt))
    }

    /// Block until request represented by `qt` is finished returning the results of this request.
    pub fn wait(&mut self, qt: QToken) -> dmtr_qresult_t {
        #[cfg(feature = "profiler")]
        timer!("catnip::wait");
        trace!("wait(): qt={:?}", qt);
        let (qd, result) = self.wait2(qt);
        dmtr_qresult_t::pack(&self.rt, result, qd, qt)
    }

    /// Block until request represented by `qt` is finished returning the file descriptor
    /// representing this request and the results of that operation.
    pub fn wait2(&mut self, qt: QToken) -> (FileDescriptor, OperationResult<RT>) {
        #[cfg(feature = "profiler")]
        timer!("catnip::wait2");
        trace!("wait2(): qt={:?}", qt);
        let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();

        // Continously call the scheduler to make progress until the future represented by `qt`
        // finishes.

        loop {
            self.poll_bg_work();
            if handle.has_completed() {
                return self.take_operation(handle);
            }
        }
    }

    pub fn wait_all_pushes(&mut self, qts: &mut Vec<QToken>) {
        #[cfg(feature = "profiler")]
        timer!("catnip::wait_all_pushes");
        trace!("wait_all_pushes(): qts={:?}", qts);
        self.poll_bg_work();
        for qt in qts.drain(..) {
            let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
            // TODO I don't understand what guarantees that this task will be done by the time we
            // get here and make this assert true.
            assert!(handle.has_completed());
            must_let!(let (_, OperationResult::Push) = self.take_operation(handle));
        }
    }

    /// Given a list of queue tokens, run all ready tasks and return the first task which has
    /// finished.
    pub fn wait_any(&mut self, qts: &[QToken]) -> (usize, dmtr_qresult_t) {
        #[cfg(feature = "profiler")]
        timer!("catnip::wait_any");
        trace!("wait_any(): qts={:?}", qts);
        loop {
            self.poll_bg_work();
            for (i, &qt) in qts.iter().enumerate() {
                let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
                if handle.has_completed() {
                    let (qd, r) = self.take_operation(handle);
                    return (i, dmtr_qresult_t::pack(&self.rt, r, qd, qt));
                }
                handle.into_raw();
            }
        }
    }

    pub fn wait_any2(&mut self, qts: &[QToken]) -> (usize, FileDescriptor, OperationResult<RT>) {
        #[cfg(feature = "profiler")]
        timer!("catnip::wait_any2");
        trace!("wait_any2(): qts={:?}", qts);

        loop {
            self.poll_bg_work();
            for (i, &qt) in qts.iter().enumerate() {
                let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
                if handle.has_completed() {
                    let (qd, r) = self.take_operation(handle);
                    return (i, qd, r);
                }
                handle.into_raw();
            }
        }
    }

    // core allocator function: register an application in a core
    pub fn core_alloc_reg_app(&mut self, core_id: u16, app_id: u16){
        assert!(app_id < (MAX_APP_NUM as u16));
        assert!(core_id < (MAX_CHANNEL_NUM as u16));
        
        let mut app_mask_map = app_to_core_bitmasks.lock().unwrap();

        if !app_mask_map.contains_key(&app_id) {
            app_mask_map.insert(app_id, 0);
        }

        let app_mask = app_mask_map.get(&app_id).unwrap();
        let new_mask = app_mask | (0x1u64 << core_id);
        app_mask_map.insert(app_id, new_mask).unwrap();
        // println!("Info: App {} Mask on core {}, 0x{:x} ", app_id, core_id, new_mask);

    }

    // For an given application: select a core to scale up
    fn select_scale_up_core(&mut self, app_id:usize){
        let mut app_mask_map = app_to_core_bitmasks.lock().unwrap();
        // 1. compute dedicate idle mask
        // 2. compute stealable mask
        // high priority could steal from low priorty tasks
        let mut dedicate_mask: u64 =  0xffffffffffffffff;
        let mut stealable_mask: u64 = 0xffffffffffffffff;
        let mut app_mask: &u64;

        let mut app_mask_1 = app_mask_map.get(&(1)).unwrap();
        let mut app_mask_2 = app_mask_map.get(&(2)).unwrap();

        dedicate_mask = app_mask_2 | app_mask_1 | self.usable_core_mask;

        if(app_id == 1){
            stealable_mask = *app_mask_1 | self.usable_core_mask;
            app_mask = app_mask_1;
        }else{
            app_mask = app_mask_2;
        }

        // let mut app_mask = app_mask_map.get(&(app_id as u16)).unwrap();
        let mut selected_core_id = 0;
        let mut if_alloc = 0;

        let mut runnning_core_count = 0;
        for x in 0..self.total_usable_cores {
            if (app_mask & (0x1u64 << x)) != 0{
                runnning_core_count += 1;
            }
        }
        let start = SystemTime::now();
        let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

        if( stealable_mask !=  0xffffffffffffffff){
            for x in 0..self.total_usable_cores {
                if (stealable_mask & (0x1u64 << x)) == 0
                {
                    // println!("Success find stealable core {} for app {}", x, app_id);
                    selected_core_id = x;
                    if_alloc = 2;
                    break;
                }
            }    
        }

        if(if_alloc > 0){
         
            let new_mask = app_mask | (0x1u64 << selected_core_id);
            app_mask_map.insert(app_id as u16, new_mask).unwrap();

            self.msg_send_channels.get(selected_core_id as usize).unwrap().send((1, app_id as u16, 0));
            // println!("Success: alloc app {} on  {}, current mask {:x}", selected_core_id, app_id, new_mask);
        }
        else{
            // println!("Waning cannot find scale up core for app {}, {:x}", app_id, app_mask);
            self.msg_send_channels.get(15 as usize).unwrap().send((3, app_id as u16, 0));
        }
    }

    // For an given application: select a core to scale down
    fn select_scale_down_core(&mut self, app_id:usize){
        let mut app_mask_map = app_to_core_bitmasks.lock().unwrap();
        let mut dedicate_mask: u64 =  0xffffffffffffffff;
        let mut stealable_mask: u64 = 0xffffffffffffffff;
        let mut app_mask: &u64;

        let mut app_mask_1 = app_mask_map.get(&(1)).unwrap();
        let mut app_mask_2 = app_mask_map.get(&(2)).unwrap();

        assert!(app_id == 1);
        app_mask = app_mask_1;

        let mut runnning_core_count = 0;
        for x in 0..self.total_usable_cores {
            if (app_mask & (0x1u64 << x)) != 0{
                runnning_core_count += 1;
            }
        }
        let start = SystemTime::now();
        let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

        if(runnning_core_count <= 1){
            // println!("Warning: Scale down -- already Minimal number for app {:x}, {}", app_mask, runnning_core_count);
            return;
        }

        // let mut app_mask = app_mask_map.get(&(app_id as u16)).unwrap();
        let mut selected_core_id = 0;
        let mut if_dealloc = 0;

        // first try to find a core in dedicate mask
        for x in (0..self.total_usable_cores).rev() {
            if (app_mask & (0x1u64 << x)) != 0
            {
                // println!("Warning: dealloc shared core {} for app {}", x, app_id);
                selected_core_id = x;
                if_dealloc = 1;
                break;
            }
        }    

        if(if_dealloc == 1){
            let new_mask = app_mask - (0x1u64 << selected_core_id);
            app_mask_map.insert(app_id as u16, new_mask).unwrap();
            self.msg_send_channels.get(selected_core_id as usize).unwrap().send((0, app_id as u16, runnning_core_count));
            // println!("Success: dealloc app {} on  {}, current mask {:x}", app_id, selected_core_id, new_mask);
        }
        else{
            // println!("Waning cannot dealloc for app {}", app_id);
        }
    }

    pub fn self_scale_down(&mut self) {
        self.select_scale_down_core(1);
    }

    pub fn prio_wait_any2(&mut self, core_id: &u16, hi_qts: &[QToken], lo_qts: &[QToken]) -> (usize, usize, FileDescriptor, OperationResult<RT>, ArrayVec<(u16, u16, u8),RECEIVE_BATCH_SIZE>) {
        #[cfg(feature = "profiler")]
        timer!("catnip::wait_any2");
        
        loop {
            // recv scale up msg from channel
            let r = self.msg_recv_channels.try_recv();
            match r{
                Ok((hint_type, app_id, cur_core_count)) => {
                    // println!("Receive scale up on target core: {}, {}", core_id, app_id);
                    let mut hint_array = ArrayVec::new();
                    hint_array.push((app_id, hint_type, cur_core_count));
                    return (100, 100, 100, OperationResult::Push, hint_array);
                },
                // empty channel, do nothing
                Err(error) => {}
            }

            let mut hint_array  = self.poll_bg_work1();

            // process the scale up msg from the NIC
            if !hint_array.is_empty(){
                // assert_eq!(hint_array.len(), 1);
                let app_id = hint_array[0].0 as usize;
                let hint_type = hint_array[0].1 as usize;
                if(hint_type == 1)
                {    self.select_scale_up_core(app_id);}
                else {
                    self.select_scale_down_core(app_id);
                }
            }

            let mut hint_array_ = ArrayVec::new();
            
            for (i, &qt) in hi_qts.iter().enumerate() {
                let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
                if handle.has_completed() {
                    let (qd, r) = self.take_operation(handle);
                    // println!("----Select Hi Handle : {}", qt);
                    return (0, i, qd, r, hint_array_);
                }
                handle.into_raw();
            }

            for (i, &qt) in lo_qts.iter().enumerate() {
                let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
                if handle.has_completed() {
                    let (qd, r) = self.take_operation(handle);
                    // println!("----Select Lo Handle : {}", qt);
                    return (1, i, qd, r, hint_array_);
                }
                handle.into_raw();
            }
        }
    }

    pub fn prio_nowait_any2(&mut self, core_id: &u16, hi_qts: &[QToken], lo_qts: &[QToken]) -> (usize, usize, FileDescriptor, OperationResult<RT>, ArrayVec<(u16, u16, u8),RECEIVE_BATCH_SIZE>) {
        #[cfg(feature = "profiler")]
        timer!("catnip::wait_any2");

            // recv scale up msg from channel
            let r = self.msg_recv_channels.try_recv();
            match r{
                Ok((hint_type, app_id, cur_core_count)) => {
                    // println!("Receive scale up on target core: {}, {}", core_id, app_id);
                    let mut hint_array = ArrayVec::new();
                    hint_array.push((app_id, hint_type, cur_core_count));
                    return (100, 100, 100, OperationResult::Push, hint_array);
                },
                // empty channel, do nothing
                Err(error) => {}
            }

            let mut hint_array  = self.poll_bg_work1();

            // process the scale up msg from the NIC
            if !hint_array.is_empty(){
                // assert_eq!(hint_array.len(), 1);
                let app_id = hint_array[0].0 as usize;
                let hint_type = hint_array[0].1 as usize;
                if(hint_type == 1)
                {    self.select_scale_up_core(app_id);}
                else {
                    self.select_scale_down_core(app_id);
                }
            }

            let mut hint_array_ = ArrayVec::new();
            
            for (i, &qt) in hi_qts.iter().enumerate() {
                let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
                if handle.has_completed() {
                    let (qd, r) = self.take_operation(handle);
                    // println!("----Select Hi Handle : {}", qt);
                    return (0, i, qd, r, hint_array_);
                }
                handle.into_raw();
            }

            for (i, &qt) in lo_qts.iter().enumerate() {
                let handle = self.rt.scheduler().from_raw_handle(qt).unwrap();
                if handle.has_completed() {
                    let (qd, r) = self.take_operation(handle);
                    // println!("----Select Lo Handle : {}", qt);
                    return (1, i, qd, r, hint_array_);
                }
                handle.into_raw();
            }

            return (300, 0, 0,  OperationResult::Push, hint_array_);
    }

    pub fn is_qd_valid(&self, _fd: FileDescriptor) -> bool {
        true
    }

    /// Given a handle representing a task in our scheduler. Return the results of this future
    /// and the file descriptor for this connection.
    ///
    /// This function will panic if the specified future had not completed or is _background_ future.
    fn take_operation(&mut self, handle: SchedulerHandle) -> (FileDescriptor, OperationResult<RT>) {
        match self.rt.scheduler().take(handle) {
            Operation::Tcp(f) => f.expect_result(),
            Operation::Udp(f) => f.expect_result(),
            Operation::Background(..) => panic!("`take_operation` attempted on background task!"),
        }
    }

    /// Scheduler will poll all futures that are ready to make progress.
    /// Then ask the runtime to receive new data which we will forward to the engine to parse and
    /// route to the correct protocol.
    fn poll_bg_work(&mut self) {
        self.rt.scheduler().poll();
        for _ in 0..MAX_RECV_ITERS {
            let (scaleUpmsg, batch) = self.rt.receive();
            if batch.is_empty() {
                break;
            }
            for pkt in batch {
                if let Err(e) = self.engine.receive(pkt) {
                    warn!("Dropped packet: {:?}", e);
                }
            }
        }
        if self.ts_iters == 0 {
            self.rt.advance_clock(Instant::now());
        }
        self.ts_iters = (self.ts_iters + 1) % TIMER_RESOLUTION;
    }

    fn poll_bg_work1(&mut self) -> ArrayVec<(u16, u16),RECEIVE_BATCH_SIZE> {
        self.rt.scheduler().poll();
        let mut scaleUpmsg = ArrayVec::new();
        //let mut batch1 = ArrayVec::new();
        for _ in 0..MAX_RECV_ITERS {
            let (scaleUpmsg1, batch) = self.rt.receive();
            if scaleUpmsg1.len() > 0{
                scaleUpmsg = scaleUpmsg1;
            }//batch1 = batch;
            if batch.is_empty() {
                break;
            }
            for pkt in batch {
                if let Err(e) = self.engine.receive(pkt) {
                    warn!("Dropped packet: {:?}", e);
                }
            }
        }
        if self.ts_iters == 0 {
            self.rt.advance_clock(Instant::now());
        }
        self.ts_iters = (self.ts_iters + 1) % TIMER_RESOLUTION;

        return scaleUpmsg;
    }
}
