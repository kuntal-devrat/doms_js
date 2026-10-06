//! ECMAScript Atomics operations on SharedArrayBuffer.

use crate::shared_array_buffer::SharedArrayBuffer;
use std::sync::atomic::Ordering;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AtomicsError {
    #[error("Index out of bounds: {0}")]
    OutOfBounds(usize),
}

pub struct Atomics;

impl Atomics {
    pub fn load(sab: &SharedArrayBuffer, index: usize) -> Result<u8, AtomicsError> {
        sab.as_atomic_slice()
            .get(index)
            .map(|a| a.load(Ordering::SeqCst))
            .ok_or(AtomicsError::OutOfBounds(index))
    }

    pub fn store(sab: &SharedArrayBuffer, index: usize, value: u8) -> Result<u8, AtomicsError> {
        let slice = sab.as_atomic_slice();
        let atomic = slice.get(index).ok_or(AtomicsError::OutOfBounds(index))?;
        atomic.store(value, Ordering::SeqCst);
        Ok(value)
    }

    pub fn add(sab: &SharedArrayBuffer, index: usize, value: u8) -> Result<u8, AtomicsError> {
        let slice = sab.as_atomic_slice();
        let atomic = slice.get(index).ok_or(AtomicsError::OutOfBounds(index))?;
        Ok(atomic.fetch_add(value, Ordering::SeqCst))
    }

    pub fn sub(sab: &SharedArrayBuffer, index: usize, value: u8) -> Result<u8, AtomicsError> {
        let slice = sab.as_atomic_slice();
        let atomic = slice.get(index).ok_or(AtomicsError::OutOfBounds(index))?;
        Ok(atomic.fetch_sub(value, Ordering::SeqCst))
    }

    pub fn compare_exchange(
        sab: &SharedArrayBuffer,
        index: usize,
        expected: u8,
        replacement: u8,
    ) -> Result<Result<u8, u8>, AtomicsError> {
        let slice = sab.as_atomic_slice();
        let atomic = slice.get(index).ok_or(AtomicsError::OutOfBounds(index))?;
        Ok(atomic.compare_exchange(expected, replacement, Ordering::SeqCst, Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomics_add_and_store() {
        let sab = SharedArrayBuffer::new(8);
        Atomics::store(&sab, 0, 10).unwrap();
        let prev = Atomics::add(&sab, 0, 5).unwrap();
        assert_eq!(prev, 10);
        assert_eq!(Atomics::load(&sab, 0).unwrap(), 15);
    }
}
