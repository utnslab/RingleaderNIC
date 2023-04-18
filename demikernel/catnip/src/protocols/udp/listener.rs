// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::protocols::ipv4;

use std::{collections::VecDeque, task::Waker};

pub struct Listener<T> {
    buf: VecDeque<(Option<ipv4::Endpoint>, T)>,
    waker: Option<Waker>,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [Listener].
impl<T> Listener<T> {
    /// Creates a new listener.
    pub fn new(buf: VecDeque<(Option<ipv4::Endpoint>, T)>, waker: Option<Waker>) -> Self {
        Self { buf, waker }
    }
    /// Pushes data to the target listener.
    pub fn len(&mut self)  -> usize{
        self.buf.len()
    }

    /// Pushes data to the target listener.
    pub fn push_data(&mut self, endpoint: Option<ipv4::Endpoint>, data: T) {
        self.buf.push_back((endpoint, data));
    }

    /// Pops data from the target listener.
    pub fn pop_data(&mut self) -> Option<(Option<ipv4::Endpoint>, T)> {
        self.buf.pop_front()
    }

    /// Takes the waker of the target listener.
    pub fn take_waker(&mut self) -> Option<Waker> {
        self.waker.take()
    }

    /// Places a waker in the target listener.
    pub fn put_waker(&mut self, waker: Option<Waker>) {
        self.waker = waker;
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Default trait implementation for [Listener].
impl<T> Default for Listener<T> {
    /// Creates a UDP socket with default values.
    fn default() -> Self {
        Self {
            buf: VecDeque::new(),
            waker: None,
        }
    }
}
