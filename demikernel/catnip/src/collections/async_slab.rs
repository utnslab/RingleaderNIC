// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use super::waker_page::{WakerPage, WakerPageRef, WAKER_PAGE_SIZE};
use futures::task::AtomicWaker;
use gen_iter::gen_iter;
use slab::Slab;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

// Adapted from https://lemire.me/blog/2018/02/21/iterating-over-set-bits-quickly/
fn iter_set_bits(mut bitset: u64) -> impl Iterator<Item = usize> {
    gen_iter!({
        while bitset != 0 {
            // `bitset & -bitset` returns a bitset with only the lowest significant bit set
            let t = bitset & bitset.wrapping_neg();
            yield bitset.trailing_zeros() as usize;
            bitset ^= t;
        }
    })
}

enum ResultFuture<F: Future> {
    Pending(F),
    Done(F::Output),
}

impl<F: Future + Unpin> Future for ResultFuture<F>
where
    F::Output: Unpin,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<()> {
        let self_ = self.get_mut();
        match self_ {
            ResultFuture::Pending(ref mut f) => {
                let result = match Future::poll(Pin::new(f), ctx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(r) => r,
                };
                *self_ = ResultFuture::Done(result);
                Poll::Ready(())
            }
            ResultFuture::Done(..) => panic!("Polled after completion"),
        }
    }
}

pub struct AsyncSlab<F: Future> {
    slab: Slab<ResultFuture<F>>,
    pages: Vec<WakerPageRef>,
    root_waker: Arc<AtomicWaker>,
}

impl<F: Future> AsyncSlab<F> {
    pub fn new() -> Self {
        Self {
            slab: Slab::new(),
            pages: vec![],
            root_waker: Arc::new(AtomicWaker::new()),
        }
    }

    fn page(&self, key: usize) -> (&WakerPageRef, usize) {
        let (page_ix, subpage_ix) = (key / WAKER_PAGE_SIZE, key % WAKER_PAGE_SIZE);
        (&self.pages[page_ix], subpage_ix)
    }

    pub fn insert(&mut self, item: F) -> usize {
        let key = self.slab.insert(ResultFuture::Pending(item));
        while key >= self.pages.len() * WAKER_PAGE_SIZE {
            self.pages.push(WakerPage::new(self.root_waker.clone()));
        }
        let (page, subpage_ix) = self.page(key);
        page.initialize(subpage_ix);
        key
    }

    pub fn len(&self) -> usize {
        self.slab.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slab.is_empty()
    }

    pub fn check_ready(&mut self, ix: usize) -> Option<F::Output> {
        let page_ix = ix / WAKER_PAGE_SIZE;
        let subpage_ix = ix % WAKER_PAGE_SIZE;
        let page = &self.pages[page_ix];
        let ready_bitset = page.get_ready();
        let ready = (ready_bitset & (1 << ix)) != 0;
        if !ready {
            return None;
        }
        let r = match self.slab.remove(ix) {
            ResultFuture::Done(out) => out,
            _ => panic!("Ready bitset and slab inconsistent"),
        };
        page.unset(subpage_ix);
        Some(r)
    }
}

impl<F: Future + Unpin> AsyncSlab<F>
where
    F::Output: Unpin,
{
    pub fn poll(&mut self, ctx: &mut Context) {
        self.root_waker.register(ctx.waker());

        for (page_ix, page) in self.pages.iter().enumerate() {
            let mut notified_bitset = page.take_notified();
            let ready_bitset = page.get_ready();

            // Unset all ready bits, since spurious notifications for completed futures would lead
            // us to poll them after completion.
            notified_bitset &= !ready_bitset;

            for subpage_ix in iter_set_bits(notified_bitset) {
                let ix = page_ix * WAKER_PAGE_SIZE + subpage_ix;
                let waker = unsafe { Waker::from_raw(page.raw_waker(subpage_ix)) };
                let mut sub_ctx = Context::from_waker(&waker);
                match Future::poll(Pin::new(&mut self.slab[ix]), &mut sub_ctx) {
                    Poll::Ready(()) => {
                        page.mark_ready(subpage_ix);
                    }
                    Poll::Pending => (),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AsyncSlab;
    use futures::{channel::oneshot, task::noop_waker_ref};
    use must_let::must_let;
    use std::{
        collections::HashMap,
        task::{Context, Poll},
    };
}
