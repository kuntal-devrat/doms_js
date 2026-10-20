//! ECMAScript Atomics operations on SharedArrayBuffer with futex-style waiting.

use crate::shared_array_buffer::SharedArrayBuffer;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AtomicsError {
    #[error("Index out of bounds: {0}")]
    OutOfBounds(usize),
    #[error("Operation not allowed: {0}")]
    NotAllowed(&'static str),
}

// Global parking lot for Atomics.wait / Atomics.notify
static FUTEX_TABLE: Mutex<Option<HashMap<usize, Arc<(Mutex<usize>, Condvar)>>>> = Mutex::new(None);

fn get_futex(slot: usize) -> Arc<(Mutex<usize>, Condvar)> {
    let mut table = FUTEX_TABLE.lock().unwrap();
    let map = table.get_or_insert_with(HashMap::new);
    map.entry(slot).or_insert_with(|| Arc::new((Mutex::new(0), Condvar::new()))).clone()
}

pub struct Atomics;

impl Atomics {
    pub fn is_lock_free(size: usize) -> bool {
        matches!(size, 1 | 2 | 4 | 8)
    }

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

    pub fn and(sab: &SharedArrayBuffer, index: usize, value: u8) -> Result<u8, AtomicsError> {
        let slice = sab.as_atomic_slice();
        let atomic = slice.get(index).ok_or(AtomicsError::OutOfBounds(index))?;
        Ok(atomic.fetch_and(value, Ordering::SeqCst))
    }

    pub fn or(sab: &SharedArrayBuffer, index: usize, value: u8) -> Result<u8, AtomicsError> {
        let slice = sab.as_atomic_slice();
        let atomic = slice.get(index).ok_or(AtomicsError::OutOfBounds(index))?;
        Ok(atomic.fetch_or(value, Ordering::SeqCst))
    }

    pub fn xor(sab: &SharedArrayBuffer, index: usize, value: u8) -> Result<u8, AtomicsError> {
        let slice = sab.as_atomic_slice();
        let atomic = slice.get(index).ok_or(AtomicsError::OutOfBounds(index))?;
        Ok(atomic.fetch_xor(value, Ordering::SeqCst))
    }

    pub fn exchange(sab: &SharedArrayBuffer, index: usize, value: u8) -> Result<u8, AtomicsError> {
        let slice = sab.as_atomic_slice();
        let atomic = slice.get(index).ok_or(AtomicsError::OutOfBounds(index))?;
        Ok(atomic.swap(value, Ordering::SeqCst))
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

    /// Futex-style wait on index until notified or timeout expires.
    /// Returns "not-equal", "timed-out", or "ok".
    pub fn wait(
        sab: &SharedArrayBuffer,
        index: usize,
        expected: i32,
        timeout: Option<Duration>,
    ) -> Result<&'static str, AtomicsError> {
        let slice = sab.as_atomic_slice();
        if index + 4 > slice.len() {
            return Err(AtomicsError::OutOfBounds(index));
        }

        // Int32 load
        let b0 = slice[index].load(Ordering::SeqCst);
        let b1 = slice[index + 1].load(Ordering::SeqCst);
        let b2 = slice[index + 2].load(Ordering::SeqCst);
        let b3 = slice[index + 3].load(Ordering::SeqCst);
        let current = i32::from_le_bytes([b0, b1, b2, b3]);

        if current != expected {
            return Ok("not-equal");
        }

        let futex = get_futex(index);
        let mut waiters = futex.0.lock().unwrap();
        *waiters += 1;

        let res = match timeout {
            Some(t) => {
                let (g, wait_res) = futex.1.wait_timeout(waiters, t).unwrap();
                waiters = g;
                if wait_res.timed_out() { Ok("timed-out") } else { Ok("ok") }
            }
            None => {
                waiters = futex.1.wait(waiters).unwrap();
                Ok("ok")
            }
        };
        *waiters -= 1;
        drop(waiters);
        res
    }

    /// Wake up to `count` waiting threads on the given index.
    pub fn notify(
        sab: &SharedArrayBuffer,
        index: usize,
        count: usize,
    ) -> Result<usize, AtomicsError> {
        let slice = sab.as_atomic_slice();
        if index >= slice.len() {
            return Err(AtomicsError::OutOfBounds(index));
        }

        let futex = get_futex(index);
        let waiters = futex.0.lock().unwrap();
        let to_wake = (*waiters).min(count);
        if to_wake > 0 {
            if count == 1 {
                futex.1.notify_one();
            } else {
                futex.1.notify_all();
            }
        }
        Ok(to_wake)
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
        assert!(Atomics::is_lock_free(4));
    }

    #[test]
    fn test_atomics_bitwise_ops() {
        let sab = SharedArrayBuffer::new(8);
        Atomics::store(&sab, 0, 0b1010).unwrap();
        assert_eq!(Atomics::and(&sab, 0, 0b1100).unwrap(), 0b1010);
        assert_eq!(Atomics::load(&sab, 0).unwrap(), 0b1000);
        assert_eq!(Atomics::or(&sab, 0, 0b0011).unwrap(), 0b1000);
        assert_eq!(Atomics::load(&sab, 0).unwrap(), 0b1011);
        assert_eq!(Atomics::xor(&sab, 0, 0b1111).unwrap(), 0b1011);
        assert_eq!(Atomics::load(&sab, 0).unwrap(), 0b0100);
        assert_eq!(Atomics::exchange(&sab, 0, 42).unwrap(), 0b0100);
        assert_eq!(Atomics::load(&sab, 0).unwrap(), 42);
    }

    #[test]
    fn test_atomics_wait_notify_not_equal() {
        let sab = SharedArrayBuffer::new(8);
        let res = Atomics::wait(&sab, 0, 999, Some(Duration::from_millis(10))).unwrap();
        assert_eq!(res, "not-equal");
    }
}
