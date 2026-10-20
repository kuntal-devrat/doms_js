//! SharedArrayBuffer backing implementation.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

#[derive(Debug, Clone)]
pub struct SharedArrayBuffer {
    data: Arc<Vec<AtomicU8>>,
    detached: Arc<AtomicBool>,
}

impl SharedArrayBuffer {
    pub fn new(byte_length: usize) -> Self {
        Self::try_new(byte_length).unwrap_or_else(|_| Self {
            data: Arc::new(Vec::new()),
            detached: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn try_new(byte_length: usize) -> Result<Self, &'static str> {
        // Guard against absurd allocations from test262 (e.g. 16GB)
        if byte_length > 64 * 1024 * 1024 {
            return Err("RangeError: Array buffer allocation limit exceeded (max 64MB)");
        }
        let mut vec = Vec::new();
        if vec.try_reserve(byte_length).is_err() {
            return Err("RangeError: Out of memory");
        }
        for _ in 0..byte_length {
            vec.push(AtomicU8::new(0));
        }
        Ok(Self { data: Arc::new(vec), detached: Arc::new(AtomicBool::new(false)) })
    }

    pub fn is_detached(&self) -> bool {
        self.detached.load(Ordering::SeqCst)
    }

    pub fn detach(&self) {
        self.detached.store(true, Ordering::SeqCst);
    }

    pub fn byte_length(&self) -> usize {
        if self.is_detached() { 0 } else { self.data.len() }
    }

    pub fn get(&self, index: usize) -> Option<u8> {
        if self.is_detached() {
            None
        } else {
            self.data.get(index).map(|a| a.load(Ordering::SeqCst))
        }
    }

    pub fn set(&self, index: usize, value: u8) -> bool {
        if self.is_detached() {
            false
        } else if let Some(atomic) = self.data.get(index) {
            atomic.store(value, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn as_atomic_slice(&self) -> &[AtomicU8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_array_buffer() {
        let sab = SharedArrayBuffer::new(16);
        assert_eq!(sab.byte_length(), 16);
        assert_eq!(sab.get(0), Some(0));
        sab.set(0, 42);
        assert_eq!(sab.get(0), Some(42));
    }
}
