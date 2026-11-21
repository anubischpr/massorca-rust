//! The newest output bytes of a terminal, addressed by absolute offset (decision 0034).

use std::collections::VecDeque;

/// Bytes of output kept per terminal.
pub const RING_CAPACITY: usize = 1024 * 1024;

/// The newest `capacity` bytes of a stream; the first byte ever pushed is offset 0.
#[derive(Debug)]
pub struct ByteRing {
    capacity: usize,
    buf: VecDeque<u8>,
    /// Total bytes ever pushed.
    end: u64,
}

impl ByteRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buf: VecDeque::new(),
            end: 0,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.end += bytes.len() as u64;
        // A push longer than the ring only ever keeps its tail.
        let keep = &bytes[bytes.len().saturating_sub(self.capacity)..];
        let overflow = (self.buf.len() + keep.len()).saturating_sub(self.capacity);
        self.buf.drain(..overflow);
        self.buf.extend(keep);
    }

    pub fn start_offset(&self) -> u64 {
        self.end - self.buf.len() as u64
    }

    pub fn end_offset(&self) -> u64 {
        self.end
    }

    /// The bytes from `offset` to the end, or `None` once `offset` is evicted or past the end.
    pub fn read_from(&self, offset: u64) -> Option<Vec<u8>> {
        if offset < self.start_offset() || offset > self.end {
            return None;
        }
        let skip = (offset - self.start_offset()) as usize;
        Some(self.buf.range(skip..).copied().collect())
    }
}
