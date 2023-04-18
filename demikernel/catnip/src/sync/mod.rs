// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

mod threadsafe;
mod threadunsafe;

pub use self::threadunsafe::{SharedWaker, WakerU64};
