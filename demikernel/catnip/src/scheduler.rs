// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

//! Implementation of our efficient, single-threaded task scheduler.
//!
//! Our scheduler holds [Operation]s in a memory slab for short
//! lived tasks on in the heap for longer lived tasks. Notice the slab is pinned memory as we must
//! make sure Futures don't move (Pin). The scheduler is a single-threaded runtime which attempts
//! to run all tasks which are available to run again. The Background tasks are always ready to poll
//! again.
//!
//! As background tasks are polled, they notify task in our scheduler via the WakerPage mechanism
//! so the scheduler only polls (schedules and runs) tasks that it knows are ready to run.

// TODO: Our safety here is very precarious.
// We should separate the scheduler into two components.
// 1) A single Scheduler owned by the top level loop. This can take out finished values and poll.
// 2) A cloneable half that's given to the runtime. This can insert new values and drop handles.
//
use crate::{
    collections::waker_page::{WakerPage, WakerPageRef, WAKER_PAGE_SIZE},
    protocols::{tcp::operations::TcpOperation, udp::UdpOperation},
    runtime::Runtime,
    sync::SharedWaker,
};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use bit_iter::*;
use unicycle::pin_slab::PinSlab;

/// The different types of operations our [Scheduler] can hold and multiplex between.
///
/// [Operation]s are tasks (top-level futures which are managed by our scheduler). This is
/// the granularity of our scheduling (our schedulable units).
///
/// Most operations are stored by our scheduler on a preallocated [PinSlab](unicycle::pin_slab::PinSlab)
/// to avoid expensive allocation, these represent shorter-lived work.
///
/// [Background](Operation::Background) tasks are heap-allocated as they are expected to live
/// long so we allocate them on the heap.
pub enum Operation<RT: Runtime> {
    Tcp(TcpOperation<RT>),
    Udp(UdpOperation<RT>),

    // These are expected to have long lifetimes and be large enough to justify another allocation.
    Background(Pin<Box<dyn Future<Output = ()>>>),
}

/// Simple wrapper which calls the corresponding [poll](Future::poll) method for each enum variant's
/// type.
impl<RT: Runtime> Future for Operation<RT> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        match self.get_mut() {
            Operation::Tcp(ref mut f) => Future::poll(Pin::new(f), ctx),
            Operation::Udp(ref mut f) => Future::poll(Pin::new(f), ctx),
            Operation::Background(ref mut f) => Future::poll(Pin::new(f), ctx),
        }
    }
}

impl<T: Into<TcpOperation<RT>>, RT: Runtime> From<T> for Operation<RT> {
    fn from(f: T) -> Self {
        Operation::Tcp(f.into())
    }
}

/// Handle returned by the scheduler once a future has been added. This handle uniquely identifies
/// a future to the scheduler.
#[allow(rustdoc::private_intra_doc_links)]
pub struct SchedulerHandle {
    /// Key specifies the location of the corresponding future in the scheduler memory slab.
    key: Option<u64>,
    /// Page in which the future corresponding to this handle lives in.
    waker_page: WakerPageRef,
}

impl SchedulerHandle {
    /// Returns if the future represented by this handle has completed.
    pub fn has_completed(&self) -> bool {
        let subpage_ix = self.key.unwrap() as usize % WAKER_PAGE_SIZE;
        self.waker_page.has_completed(subpage_ix)
    }

    /// Returns the raw key for this handle consuming the SchedulerHandle.
    pub fn into_raw(mut self) -> u64 {
        self.key.take().unwrap()
    }
}

impl Drop for SchedulerHandle {
    /// Decrease the reference count for handles pointing to this future.
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            let subpage_ix = key as usize % WAKER_PAGE_SIZE;
            self.waker_page.mark_dropped(subpage_ix);
        }
    }
}

/// The scheduler
/// runs on a single thread multiplexing between all available work.
pub struct Scheduler<F: Future<Output = ()> + Unpin> {
    inner: Rc<RefCell<Inner<F>>>,
}

impl<F: Future<Output = ()> + Unpin> Clone for Scheduler<F> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<F: Future<Output = ()> + Unpin> Default for Scheduler<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Future<Output = ()> + Unpin> Scheduler<F> {
    /// New empty scheduler with default settings.
    pub fn new() -> Self {
        let inner = Inner {
            slab: PinSlab::new(),
            pages: vec![],
            root_waker: SharedWaker::new(),
        };
        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }

    /// Given a handle representing a future, remove the future from the scheduler returning it.
    pub fn take(&self, mut handle: SchedulerHandle) -> F {
        let mut inner = self.inner.borrow_mut();
        let key = handle.key.take().unwrap();
        let (page, subpage_ix) = inner.page(key);
        assert!(!page.was_dropped(subpage_ix));
        page.clear(subpage_ix);
        // println!("page take: {}, {}, {}", key, subpage_ix, key%WAKER_PAGE_SIZE as u64);
        inner.slab.remove_unpin((key%WAKER_PAGE_SIZE as u64 ) as usize).unwrap()
    }

    /// Given the raw `key` representing this future return a proper handle.
    pub fn from_raw_handle(&self, key: u64) -> Option<SchedulerHandle> {
        let inner = self.inner.borrow();
        inner.slab.get((key%WAKER_PAGE_SIZE as u64 )  as usize)?;
        let (page, _) = inner.page(key);
        let handle = SchedulerHandle {
            key: Some(key),
            waker_page: page.clone(),
        };
        Some(handle)
    }

    /// Insert a new task into our scheduler returning a handle corresponding to it.
    pub fn insert(&self, future: F) -> SchedulerHandle {
        let mut inner = self.inner.borrow_mut();
        let key = inner.insert(future);
        let (page, _) = inner.page(key);
        SchedulerHandle {
            key: Some(key),
            waker_page: page.clone(),
        }
    }


    pub fn insert_page(&self, future: F, page:usize) -> SchedulerHandle {
        let mut inner = self.inner.borrow_mut();
        let key = inner.insert_page(future, page);
        let (page, _) = inner.page(key);
        SchedulerHandle {
            key: Some(key),
            waker_page: page.clone(),
        }
    }

    /// Poll all futures which are ready to run again. Tasks in our scheduler are notified when
    /// relevant data or events happen. The relevant event have callback function (the waker) which
    /// they can invoke to notify the scheduler that future should be polled again.
    pub fn poll(&self) {
        let mut inner = self.inner.borrow_mut();
        // inner.root_waker.register(ctx.waker());

        // TODO rewrite this loop to use high-level iterators instead of indexes.
        // Iterate through all our pages finding the tasks that are ready to be polled again
        // (notified) and dropped tasks which can be removed.
        // println!("Enter sche----");
        // let  mut check_if = 0;
        for page_ix in 0..inner.pages.len() {
            let (notified, dropped) = {
                let page = &mut inner.pages[page_ix];
                (page.take_notified(), page.take_dropped())
            };
            // Non-zero means at least one future in this page should be polled.
            if notified != 0 {
                // check_if =1;
                // Iterate through this page's bit vector polling the futures that are ready.
                for subpage_ix in BitIter::from(notified) {
                    // println!("Check {}, {}", page_ix, subpage_ix);
                    if subpage_ix != 0 {
                        // Get future using our page indices and poll it!
                        let ix = page_ix * WAKER_PAGE_SIZE + subpage_ix;
                        let waker =
                            unsafe { Waker::from_raw(inner.pages[page_ix].raw_waker(subpage_ix)) };
                        let mut sub_ctx = Context::from_waker(&waker);
                        // println!("cal ori {}, offset {}", ix, ix%WAKER_PAGE_SIZE);
                        let pinned_ref = inner.slab.get_pin_mut((ix%WAKER_PAGE_SIZE ) as usize ).unwrap();
                        let pinned_ptr = unsafe { Pin::into_inner_unchecked(pinned_ref) as *mut _ };

                        drop(inner);
                        let pinned_ref = unsafe { Pin::new_unchecked(&mut *pinned_ptr) };
                        let poll_result = { Future::poll(pinned_ref, &mut sub_ctx) };
                        inner = self.inner.borrow_mut();

                        match poll_result {
                            Poll::Ready(()) => inner.pages[page_ix].mark_completed(subpage_ix),
                            Poll::Pending => (),
                        }
                    }
                }
                break;
            }
            if dropped != 0 {
                println!("Entered dropped");
                for subpage_ix in BitIter::from(dropped) {
                    if subpage_ix != 0 {
                        let ix = page_ix * WAKER_PAGE_SIZE + subpage_ix;
                        inner.slab.remove((ix%WAKER_PAGE_SIZE ) as usize);
                        inner.pages[page_ix].clear(subpage_ix);
                    }
                }
            }
        }

        // if(check_if == 1){
        //     println!("------------");
        // }
    }
}

/// Actual data used by [Scheduler].
struct Inner<F: Future<Output = ()> + Unpin> {
    /// Tasks are held by the scheduler in this memory slab.
    slab: PinSlab<F>,
    /// Holds the current status of which tasks are ready to be polled (scheduled) again.
    /// The statuses are arranged in pages.
    pages: Vec<WakerPageRef>,
    root_waker: SharedWaker,
}

impl<F: Future<Output = ()> + Unpin> Inner<F> {
    /// Our pages hold 64 contiguous future wakers, so we can do simple arithmetic to access the
    /// correct page as well as the index within page.
    /// Given the `key` representing a future, return a reference to that page, `WakerPageRef`. And
    /// the index _within_ that page (usize).
    fn page(&self, key: u64) -> (&WakerPageRef, usize) {
        let key = key as usize;
        let (page_ix, subpage_ix) = (key / WAKER_PAGE_SIZE, key % WAKER_PAGE_SIZE);
        (&self.pages[page_ix], subpage_ix)
    }

    /// Insert a future into our scheduler returning an integer key representing this future. This
    /// key is used to index into the slab for accessing the future.
    fn insert(&mut self, future: F) -> u64 {
        let key = self.slab.insert(future);

        // Add a new page to hold this future's status if the current page is filled.
        while key >= self.pages.len() * WAKER_PAGE_SIZE {
            self.pages.push(WakerPage::new(self.root_waker.clone()));
        }
        // println!("INsert SChedduler: {}", self.pages.len());
        let (page, subpage_ix) = self.page(key as u64);
        page.initialize(subpage_ix);
        key as u64
    }

    /// Insert a future into our scheduler returning an integer key representing this future. This
    /// key is used to index into the slab for accessing the future.
    fn insert_page(&mut self, future: F, prio:usize) -> u64 {
        let key = self.slab.insert(future) + prio *  WAKER_PAGE_SIZE;
        // Add a new page to hold this future's status if the current page is filled.
        while key >= self.pages.len() * WAKER_PAGE_SIZE {
            self.pages.push(WakerPage::new(self.root_waker.clone()));
        }
        // println!("Prio {} INsert SChedduler: {}, {}",prio, key, self.pages.len());
        let (page, subpage_ix) = self.page(key as u64);
        page.initialize(subpage_ix);
        key as u64
    }
}
