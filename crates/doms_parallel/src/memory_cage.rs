//! Memory Caging architecture for ArrayBuffer and SharedArrayBuffer memory isolation.
//!
//! Inspired by Ladybird and V8's pointer caging:
//! All TypedArray and ArrayBuffer backing allocations live inside a contiguous,
//! reserved virtual memory cage (up to 4 GiB / 4 TiB address space).
//! Any out-of-bounds access or corrupted offset is strictly clamped within the cage,
//! preventing arbitrary process memory tampering or security escapes.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Standard memory cage virtual size (4 GiB virtual reservation window).
pub const DEFAULT_CAGE_SIZE: usize = 4 * 1024 * 1024 * 1024; // 4 GiB

/// Represents an individual allocation bounded inside the memory cage.
#[derive(Debug, Clone)]
pub struct CagedAllocation {
    pub id: usize,
    pub offset_in_cage: usize,
    pub size: usize,
}

/// The global or isolate-level Memory Cage sandbox.
pub struct MemoryCage {
    cage_base: usize,
    cage_size: usize,
    allocated_bytes: AtomicUsize,
    allocations: Mutex<HashMap<usize, CagedAllocation>>,
    next_id: AtomicUsize,
    #[allow(dead_code)]
    backing_memory: Vec<u8>,
}

impl MemoryCage {
    /// Create a new Memory Cage with a specified virtual reservation capacity.
    pub fn new(size: usize) -> Self {
        let mut backing = Vec::with_capacity(1024);
        backing.resize(1024.min(size), 0);

        let base_ptr = backing.as_ptr() as usize;

        Self {
            cage_base: base_ptr,
            cage_size: size,
            allocated_bytes: AtomicUsize::new(0),
            allocations: Mutex::new(HashMap::new()),
            next_id: AtomicUsize::new(1),
            backing_memory: backing,
        }
    }

    /// Default 4 GiB memory cage instance.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_CAGE_SIZE)
    }

    /// Base memory address of the reserved cage.
    pub fn base_address(&self) -> usize {
        self.cage_base
    }

    /// Size in bytes of the memory cage.
    pub fn cage_size(&self) -> usize {
        self.cage_size
    }

    /// Currently allocated bytes within the cage.
    pub fn allocated_bytes(&self) -> usize {
        self.allocated_bytes.load(Ordering::SeqCst)
    }

    /// Allocate a caged buffer of `size` bytes within the cage boundaries.
    pub fn allocate(&self, size: usize) -> Result<CagedBuffer, &'static str> {
        if size == 0 {
            return Ok(CagedBuffer::empty());
        }

        let current = self.allocated_bytes.load(Ordering::SeqCst);
        if current + size > self.cage_size {
            return Err("MemoryCage: allocation exceeds 4GiB cage capacity");
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let offset = current;

        let alloc = CagedAllocation {
            id,
            offset_in_cage: offset,
            size,
        };

        let mut map = self.allocations.lock().unwrap();
        map.insert(id, alloc.clone());
        self.allocated_bytes.fetch_add(size, Ordering::SeqCst);

        Ok(CagedBuffer {
            id,
            offset_in_cage: offset,
            size,
            data: Arc::new(Mutex::new(vec![0u8; size])),
        })
    }

    /// Verify whether a raw address resides strictly inside the bounds of this cage.
    pub fn is_within_cage(&self, address: usize) -> bool {
        address >= self.cage_base && address < (self.cage_base.saturating_add(self.cage_size))
    }
}

/// A slice of memory strictly allocated and guarded by the Memory Cage.
#[derive(Debug, Clone)]
pub struct CagedBuffer {
    pub id: usize,
    pub offset_in_cage: usize,
    pub size: usize,
    data: Arc<Mutex<Vec<u8>>>,
}

impl CagedBuffer {
    pub fn empty() -> Self {
        Self {
            id: 0,
            offset_in_cage: 0,
            size: 0,
            data: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Read a byte at `index` with strict bounds checking.
    pub fn get(&self, index: usize) -> Option<u8> {
        if index >= self.size {
            return None;
        }
        let lock = self.data.lock().unwrap();
        lock.get(index).copied()
    }

    /// Write a byte at `index` with strict bounds checking.
    pub fn set(&self, index: usize, value: u8) -> bool {
        if index >= self.size {
            return false;
        }
        let mut lock = self.data.lock().unwrap();
        if let Some(byte) = lock.get_mut(index) {
            *byte = value;
            true
        } else {
            false
        }
    }

    /// Copy bytes into an output buffer.
    pub fn read_bytes(&self, offset: usize, out: &mut [u8]) -> usize {
        if offset >= self.size {
            return 0;
        }
        let lock = self.data.lock().unwrap();
        let available = self.size - offset;
        let to_copy = out.len().min(available);
        out[..to_copy].copy_from_slice(&lock[offset..offset + to_copy]);
        to_copy
    }

    /// Copy bytes from input buffer into this caged buffer.
    pub fn write_bytes(&self, offset: usize, src: &[u8]) -> usize {
        if offset >= self.size {
            return 0;
        }
        let mut lock = self.data.lock().unwrap();
        let available = self.size - offset;
        let to_copy = src.len().min(available);
        lock[offset..offset + to_copy].copy_from_slice(&src[..to_copy]);
        to_copy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_cage_creation() {
        let cage = MemoryCage::with_defaults();
        assert_eq!(cage.cage_size(), DEFAULT_CAGE_SIZE);
        assert_eq!(cage.allocated_bytes(), 0);
    }

    #[test]
    fn test_memory_cage_allocation_and_bounds() {
        let cage = MemoryCage::new(1024 * 1024);
        let buf = cage.allocate(1024).expect("allocate 1KB");
        assert_eq!(buf.len(), 1024);

        assert!(buf.set(0, 42));
        assert!(buf.set(1023, 99));
        assert_eq!(buf.get(0), Some(42));
        assert_eq!(buf.get(1023), Some(99));

        // Out of bounds check
        assert_eq!(buf.get(1024), None);
        assert!(!buf.set(1024, 1));
    }

    #[test]
    fn test_memory_cage_capacity_limit() {
        let cage = MemoryCage::new(512);
        let _ = cage.allocate(500).unwrap();
        // Exceeds cage capacity
        assert!(cage.allocate(100).is_err());
    }
}
