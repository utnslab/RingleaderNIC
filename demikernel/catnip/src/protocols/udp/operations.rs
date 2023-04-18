// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use futures_intrusive::buffer::RingBuf;

use super::listener::Listener;

use crate::protocols::ipv4::endpoint::Ipv4Endpoint;
use crate::{fail::Fail, file_table::FileDescriptor, operations::ResultFuture, runtime::Runtime};

use crate::{operations::OperationResult, protocols::ipv4};

use std::collections::VecDeque;
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

//==============================================================================
// Constants & Structures
//==============================================================================

/// Future for Pop Operation

pub struct PopFuture<RT: Runtime> {
    /// File descriptor.
    fd: FileDescriptor,
    /// Listener.
    listener: Result<Rc<RefCell<Listener<RT::Buf>>>, Fail>,
}

pub struct PopBatchFuture<RT: Runtime> {
    /// File descriptor.
    fd: FileDescriptor,
    /// Listener.
    listener: Result<Rc<RefCell<Listener<RT::Buf>>>, Fail>,
}

/// Operations on UDP Layer
pub enum UdpOperation<RT: Runtime> {
    Connect(FileDescriptor, Result<(), Fail>),
    Push(FileDescriptor, Result<(), Fail>),
    Dyield(FileDescriptor, Result<(), Fail>),
    Pop(ResultFuture<PopFuture<RT>>),
    PopBatch(ResultFuture<PopBatchFuture<RT>>),
}

//==============================================================================
// Associate Functions
//==============================================================================

impl<RT: Runtime> UdpOperation<RT> {
    pub fn expect_result(self) -> (FileDescriptor, OperationResult<RT>) {
        match self {
            UdpOperation::Push(fd, Err(e)) | UdpOperation::Connect(fd, Err(e)) | UdpOperation::Dyield(fd, Err(e)) => {
                (fd, OperationResult::Failed(e))
            }
            UdpOperation::Connect(fd, Ok(())) => (fd, OperationResult::Connect),
            UdpOperation::Push(fd, Ok(())) => (fd, OperationResult::Push),
            UdpOperation::Dyield(fd, Ok(())) => (fd, OperationResult::Dyield),

            UdpOperation::Pop(ResultFuture {
                future,
                done: Some(Ok((addr, bytes))),
            }) => (future.fd, OperationResult::Pop(addr, bytes)),
            UdpOperation::Pop(ResultFuture {
                future,
                done: Some(Err(e)),
            }) => (future.fd, OperationResult::Failed(e)),


            UdpOperation::PopBatch(ResultFuture {
                future,
                done: Some(Ok(vec)),
            }) => (future.fd, OperationResult::PopBatch(vec)),
            UdpOperation::PopBatch(ResultFuture {
                future,
                done: Some(Err(e)),
            }) => (future.fd, OperationResult::Failed(e)),



            _ => panic!("Future not ready"),
        }
    }
}

/// Associate functions for [PopFuture].
impl<RT: Runtime> PopFuture<RT> {
    /// Creates a future for the pop operation.
    pub fn new(fd: FileDescriptor, listener: Result<Rc<RefCell<Listener<RT::Buf>>>, Fail>) -> Self {
        Self { fd, listener }
    }
}

/// Associate functions for [PopFuture].
impl<RT: Runtime> PopBatchFuture<RT> {
    /// Creates a future for the pop operation.
    pub fn new(fd: FileDescriptor, listener: Result<Rc<RefCell<Listener<RT::Buf>>>, Fail>) -> Self {
        Self { fd, listener }
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Future trait implementation for [PopFuture].
impl<RT: Runtime> Future for PopFuture<RT> {
    type Output = Result<(Option<ipv4::Endpoint>, RT::Buf), Fail>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let self_ = self.get_mut();
        match self_.listener {
            Err(ref e) => Poll::Ready(Err(e.clone())),
            Ok(ref l) => {
                let mut listener = l.borrow_mut();
                if let Some(r) = listener.pop_data() {
                    return Poll::Ready(Ok(r));
                }
                let waker = ctx.waker();
                listener.put_waker(Some(waker.clone()));
                Poll::Pending
            }
        }
    }
}


/// Future trait implementation for [PopFuture].
impl<RT: Runtime> Future for PopBatchFuture<RT> {
    type Output = Result<Vec<(Option<ipv4::Endpoint>, RT::Buf)>, Fail>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let self_ = self.get_mut();
        match self_.listener {
            Err(ref e) => Poll::Ready(Err(e.clone())),
            Ok(ref l) => {
                let mut listener = l.borrow_mut();
                let l_len = listener.len();
                let mut bufs: Vec<(Option<Ipv4Endpoint>, <RT as Runtime>::Buf)> = Vec::with_capacity(l_len);
                if l_len > 0{
                    for i in 0 .. l_len{
                        if let Some(r) = listener.pop_data() {
                            bufs.push(r);
                        }   
                    }
                    return Poll::Ready(Ok(bufs));
                }
                let waker = ctx.waker();
                listener.put_waker(Some(waker.clone()));
                Poll::Pending
            }
        }
    }
}

/// Future trait implementation for [UdpOperation]
impl<RT: Runtime> Future for UdpOperation<RT> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<()> {
        match self.get_mut() {
            UdpOperation::Connect(..) | UdpOperation::Push(..) | UdpOperation::Dyield(..) => Poll::Ready(()),
            UdpOperation::Pop(ref mut f) => Future::poll(Pin::new(f), ctx),
            UdpOperation::PopBatch(ref mut f)  => Future::poll(Pin::new(f), ctx),
        }
    }
}
