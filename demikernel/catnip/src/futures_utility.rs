// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::fail::Fail;
use async_trait::async_trait;
use futures::future::FusedFuture;
use futures::FutureExt;
use std::future::Future;

/// Provides useful high-level future-related methods.
#[async_trait(?Send)]
pub(crate) trait UtilityMethods: Future + FusedFuture + Unpin {
    /// Transforms our current future to include a timeout. We either return the results of the
    /// future finishing or a Timeout errror. Whichever happens first.
    async fn with_timeout<Timer>(&mut self, timer: Timer) -> Result<Self::Output, Fail>
        where
        Timer: Future<Output = ()>,
        {
            futures::select! {
                result = self => Ok(result),
                _ = timer.fuse() => Err(Fail::Timeout {})
            }
        }
}

// Implement UtiliytMethods for any Future that implements Unpin and FusedFuture.
impl<F: ?Sized> UtilityMethods for F where F: Future + Unpin + FusedFuture {}
