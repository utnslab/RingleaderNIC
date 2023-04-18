// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::{fail::Fail, runtime::RuntimeBuf};

use std::{
    fmt,
    ops::{Deref, DerefMut},
    sync::Arc,
};

//==============================================================================
// Bytes
//==============================================================================

/// Non-Mutable Buffer
#[derive(Clone, Default)]
pub struct Bytes {
    buf: Option<Arc<[u8]>>,
    offset: usize,
    len: usize,
}

/// Equality of Bytes only depends on the data values and not in the offset of the buffer.
impl PartialEq for Bytes {
    fn eq(&self, rhs: &Self) -> bool {
        self[..] == rhs[..]
    }
}

impl Eq for Bytes {}

/// Runtime implementation for non-mutable buffers.
impl RuntimeBuf for Bytes {
    /// Creates an empty runtime buffer.
    fn empty() -> Self {
        Self::default()
    }

    fn from_slice(src: &[u8]) -> Self {
        let buf: Arc<[u8]> = src.into();
        Self {
            buf: Some(buf),
            offset: 0,
            len: src.len(),
        }
    }

    /// Drops the first `n` bytes of the target buffer.
    fn adjust(&mut self, n: usize) {
        if n > self.len {
            panic!("Adjusting past end of buffer: {} vs. {}", n, self.len);
        }
        self.offset += n;
        self.len -= n;
    }

    /// Drops the last `n` bytes of the target buffer.
    fn trim(&mut self, num_bytes: usize) {
        if num_bytes > self.len {
            panic!(
                "Trimming past beginning of buffer: {} vs. {}",
                num_bytes, self.len
            );
        }
        self.len -= num_bytes;
    }
}

/// Debug trait implementation for non-mutable buffers.
impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Bytes({:?})", &self[..])
    }
}

/// De-reference trait implementation for non-mutable buffers.
impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self.buf {
            None => &[],
            Some(ref buf) => &buf[self.offset..(self.offset + self.len)],
        }
    }
}

//==============================================================================
// BytesMut
//==============================================================================

pub struct BytesMut {
    buf: Arc<[u8]>,
}

/// Equality of BytesMut only depends on the data values and not in the offset of the buffer.
impl PartialEq for BytesMut {
    fn eq(&self, rhs: &Self) -> bool {
        self[..] == rhs[..]
    }
}

impl Eq for BytesMut {}

/// Mutable Buffer
impl BytesMut {
    pub fn zeroed(capacity: usize) -> Result<Self, Fail> {
        if capacity == 0 {
            return Err(Fail::Invalid {
                details: "zero-capacity buffer",
            });
        }
        Ok(Self {
            buf: unsafe { Arc::new_zeroed_slice(capacity).assume_init() },
        })
    }

    /// Converts the target mutable buffer into a non-mutable one.
    pub fn freeze(self) -> Bytes {
        Bytes {
            offset: 0,
            len: self.buf.len(),
            buf: Some(self.buf),
        }
    }
}

// Conversion trait implementation for mutable buffers.
impl From<&[u8]> for BytesMut {
    fn from(buf: &[u8]) -> Self {
        let mut b = Self::zeroed(buf.len()).unwrap();
        b[..].copy_from_slice(buf);
        b
    }
}

// Debug trait implementation for mutable buffers.
impl fmt::Debug for BytesMut {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BytesMut({:?})", &self[..])
    }
}

// Dereference trait implementation for mutable buffers.
impl Deref for BytesMut {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.buf[..]
    }
}

// Mutable dereference trait implementation for mutable buffers.
impl DerefMut for BytesMut {
    fn deref_mut(&mut self) -> &mut [u8] {
        Arc::get_mut(&mut self.buf).unwrap()
    }
}

//==============================================================================
// Unit Tests
//==============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests for buffer adjust.
    #[test]
    fn buf_adjust() {
        let data: [u8; 4] = [1, 2, 3, 4];
        let mut buf = Bytes {
            offset: 0,
            len: 4,
            buf: Some(Arc::new(data)),
        };
        buf.adjust(2);
        assert_eq!(*buf, data[2..]);
    }

    /// Tests for buffer trim.
    #[test]
    fn buf_trim() {
        let data: [u8; 4] = [1, 2, 3, 4];
        let mut buf = Bytes {
            offset: 0,
            len: 4,
            buf: Some(Arc::new(data)),
        };
        buf.trim(2);
        assert_eq!(*buf, data[..2]);
    }
}
