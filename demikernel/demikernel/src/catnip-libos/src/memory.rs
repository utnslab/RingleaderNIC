use catnip::runtime::RuntimeBuf;
use std::ptr;
use std::slice;
use std::sync::Arc;
use std::ops::Deref;
use ixy_rs::{
    ixy_tx_batch,memory_allocate_mempool,ixy_rx_batch,ixy_init,pkt_buf_alloc,pkt_buf_free, ixy_device, mempool,pkt_buf,
};

#[derive(Debug)]
pub struct Ixybuf {
    pub ptr: *mut pkt_buf,
    pub data_offset: usize,
    pub data_length: usize,
}

impl RuntimeBuf for Ixybuf{
    fn empty() -> Self {
        todo!()
    }

    fn from_slice(_: &[u8]) -> Self {
        todo!()
    }

    fn adjust(&mut self, num_bytes: usize) {
        if num_bytes > self.data_length {
            panic!("Adjusting past end of buffer: {} vs. {}", num_bytes, self.data_length);
        }
        self.data_offset += num_bytes;
        self.data_length -= num_bytes;
    }

    fn trim(&mut self, num_bytes: usize) {
        if num_bytes > self.data_length {
            panic!(
                "Trimming past beginning of buffer: {} vs. {}",
                num_bytes, self.data_length
            );
        }
        self.data_length -= num_bytes;
    }
    
    unsafe fn slice_mut(&mut self) -> &mut [u8] {
        slice::from_raw_parts_mut(self.buf_addr_phy(),2048)
    }
}

impl Clone for Ixybuf {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl Deref for Ixybuf {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe{ slice::from_raw_parts(self.buf_addr_phy(), self.len() as usize)}
        // todo!()
    }
}

// TODO: Properly handle droop function
// As for TX send, packet may be dropped before NIC fecths the data
impl Drop for Ixybuf {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        unsafe {
            // println!("Called Drop from DEmikernel");
            pkt_buf_free(self.ptr);
        }
        self.ptr = ptr::null_mut();
    }
}

impl Ixybuf {
    pub fn buf_addr_phy(&self) -> *mut u8 {
        unsafe {
            let struct_ptr = (self.ptr) as *mut u8;
            let buf_ptr = struct_ptr.offset((64 + self.data_offset) as isize) as *mut u8;
            buf_ptr
        }
    }

    pub fn len(&self) -> usize {
        unsafe {self.data_length as usize }
    }

    pub fn bufsize(&self) -> u32 {
        unsafe {
            let buf_size = (*self.ptr).size as u32;
            buf_size
        }
    }

    pub fn setbufsize(&self, bufsize: usize){
        unsafe {
            (*self.ptr).size = bufsize as u32;
        }
    }

    pub fn forward_offset(&mut self, num_bytes: usize){
        if num_bytes > self.data_offset {
            panic!("forward_offset past start of buffer: {} vs. {}", num_bytes, self.data_offset);
        }
        self.data_offset -= num_bytes;
    }

    pub fn update_length(&mut self, num_bytes: usize){
        self.data_length += num_bytes;
    }
}
