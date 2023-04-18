// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
#![cfg_attr(feature = "strict", deny(warnings))]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(const_panic, const_alloc_layout)]
#![feature(const_mut_refs, const_type_name)]
#![feature(generators, generator_trait)]
#![feature(new_uninit)]
#![feature(maybe_uninit_uninit_array, maybe_uninit_extra, maybe_uninit_ref)]
#![feature(never_type)]
#![feature(raw)]
#![feature(try_blocks)]
#![deny(clippy::all)]
#![recursion_limit = "512"]
#![feature(test)]
#![feature(min_type_alias_impl_trait)]

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate log;

#[macro_use]
extern crate derive_more;

#[macro_use]
extern crate lazy_static;

pub mod collections;
pub mod engine;
pub mod fail;
pub mod file_table;
mod futures_utility;
pub mod interop;
pub mod libos;
pub mod logging;
pub mod operations;
pub mod options;
pub mod protocols;
pub mod runtime;
pub mod scheduler;
pub mod sync;
pub mod test_helpers;
pub mod timer;
