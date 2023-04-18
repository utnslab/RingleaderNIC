// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use futures::task::AtomicWaker;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    task::Waker,
};

pub struct SharedWaker(Arc<AtomicWaker>);

impl Clone for SharedWaker {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl SharedWaker {
    #[allow(unused)]
    pub fn new() -> Self {
        Self(Arc::new(AtomicWaker::new()))
    }

    #[allow(unused)]
    pub fn register(&self, waker: &Waker) {
        self.0.register(waker);
    }

    #[allow(unused)]
    pub fn wake(&self) {
        self.0.wake();
    }
}

pub struct WakerU64(AtomicU64);

impl WakerU64 {
    #[allow(unused)]
    pub fn new(val: u64) -> Self {
        WakerU64(AtomicU64::new(val))
    }

    #[allow(unused)]
    pub fn fetch_or(&self, val: u64) {
        self.0.fetch_or(val, Ordering::SeqCst);
    }

    #[allow(unused)]
    pub fn fetch_and(&self, val: u64) {
        self.0.fetch_and(val, Ordering::SeqCst);
    }

    #[allow(unused)]
    pub fn fetch_add(&self, val: u64) -> u64 {
        self.0.fetch_add(val, Ordering::SeqCst)
    }

    #[allow(unused)]
    pub fn fetch_sub(&self, val: u64) -> u64 {
        self.0.fetch_sub(val, Ordering::SeqCst)
    }

    #[allow(unused)]
    pub fn load(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }

    #[allow(unused)]
    pub fn swap(&self, val: u64) -> u64 {
        self.0.swap(val, Ordering::SeqCst)
    }
}
