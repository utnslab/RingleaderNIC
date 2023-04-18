// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use ixy_rs::*;
use std::env;
use std::ffi::CString;
use std::mem::MaybeUninit;
use std::time::Duration;
use ixy_rs::test_link_success;

fn main() {
    unsafe {

    test_link_success();
    let mempool = memory_allocate_mempool(512, 10);
    let buf = pkt_buf_alloc(mempool);
    
    }
}
