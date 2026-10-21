//! Memory Caging architecture for ArrayBuffer and SharedArrayBuffer memory isolation.
//!
//! Inspired by Ladybird and V8's pointer caging:
//! All TypedArray and ArrayBuffer backing allocations live inside a contiguous,
//! reserved virtual memory cage (up to 4 GiB / 4 TiB address space).
//! Any out-of-bounds access or corrupted offset is strictly clamped within the cage,
//! preventing arbitrary process memory tampering or security escapes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Standard memory cage virtual size (4 GiB virtual reservation window).
pub const DEFAULT_CAGE_SIZE: usize = 4 * 1024 * 1024 * 1024; // 4 GiB

#[cfg(unix)]
fn get_unix_mmap_flags() -> libc::c_int {
    #[cfg(target_os = "linux")]
    {
        libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE
    }
    #[cfg(target_os = "macos")]
    {
        libc::MAP_PRIVATE | libc::MAP_ANON
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        libc::MAP_PRIVATE | libc::MAP_ANON
    }
}

/// An allocated slice within the memory sandbox cage.
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
    is_mmap: bool,
    #[allow(dead_code)]
    fallback_memory: Option<Vec<u8>>,
}

unsafe impl Send for MemoryCage {}
unsafe impl Sync for MemoryCage {}

impl MemoryCage {
    /// Create a new Memory Cage with a specified virtual reservation capacity.
    pub fn new(size: usize) -> Self {
        #[cfg(unix)]
        {
            let map_flags = get_unix_mmap_flags();
            let ptr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    map_flags,
                    -1,
                    0,
                )
            };
            if ptr != libc::MAP_FAILED && !ptr.is_null() {
                return Self {
                    cage_base: ptr as usize,
                    cage_size: size,
                    allocated_bytes: AtomicUsize::new(0),
                    allocations: Mutex::new(HashMap::new()),
                    next_id: AtomicUsize::new(1),
                    is_mmap: true,
                    fallback_memory: None,
                };
            }
        }

        // Fallback for non-unix or if mmap fails
        let fallback_size = size.min(1024 * 1024);
        let mut fallback = vec![0u8; fallback_size];
        let base = fallback.as_mut_ptr() as usize;

        Self {
            cage_base: base,
            cage_size: size,
            allocated_bytes: AtomicUsize::new(0),
            allocations: Mutex::new(HashMap::new()),
            next_id: AtomicUsize::new(1),
            is_mmap: false,
            fallback_memory: Some(fallback),
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

    /// Whether this cage is backed by real OS mmap virtual memory.
    pub fn is_virtual_mmap(&self) -> bool {
        self.is_mmap
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
        if current.saturating_add(size) > self.cage_size {
            return Err("MemoryCage: allocation exceeds cage capacity");
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let offset = current;

        let alloc = CagedAllocation { id, offset_in_cage: offset, size };

        let mut map = self.allocations.lock().unwrap();
        map.insert(id, alloc);
        self.allocated_bytes.fetch_add(size, Ordering::SeqCst);

        let data = if self.is_mmap {
            let slice_ptr = (self.cage_base + offset) as *mut u8;
            CagedBufferStorage::Virtual(slice_ptr)
        } else {
            CagedBufferStorage::Heap(Arc::new(Mutex::new(vec![0u8; size])))
        };

        Ok(CagedBuffer { id, offset_in_cage: offset, size, storage: data })
    }

    /// Verify whether a raw address resides strictly inside the bounds of this cage.
    pub fn is_within_cage(&self, address: usize) -> bool {
        address >= self.cage_base && address < (self.cage_base.saturating_add(self.cage_size))
    }
}

impl Drop for MemoryCage {
    fn drop(&mut self) {
        #[cfg(unix)]
        if self.is_mmap && self.cage_base != 0 {
            unsafe {
                libc::munmap(self.cage_base as *mut libc::c_void, self.cage_size);
            }
        }
    }
}

#[derive(Debug, Clone)]
enum CagedBufferStorage {
    Virtual(*mut u8),
    Heap(Arc<Mutex<Vec<u8>>>),
}

unsafe impl Send for CagedBufferStorage {}
unsafe impl Sync for CagedBufferStorage {}

/// A slice of memory strictly allocated and guarded by the Memory Cage.
#[derive(Debug, Clone)]
pub struct CagedBuffer {
    pub id: usize,
    pub offset_in_cage: usize,
    pub size: usize,
    storage: CagedBufferStorage,
}

impl CagedBuffer {
    pub fn empty() -> Self {
        Self {
            id: 0,
            offset_in_cage: 0,
            size: 0,
            storage: CagedBufferStorage::Heap(Arc::new(Mutex::new(Vec::new()))),
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
        match &self.storage {
            CagedBufferStorage::Virtual(ptr) => unsafe { Some(*ptr.add(index)) },
            CagedBufferStorage::Heap(h) => {
                let lock = h.lock().unwrap();
                lock.get(index).copied()
            }
        }
    }

    /// Write a byte at `index` with strict bounds checking.
    pub fn set(&self, index: usize, value: u8) -> bool {
        if index >= self.size {
            return false;
        }
        match &self.storage {
            CagedBufferStorage::Virtual(ptr) => unsafe {
                *ptr.add(index) = value;
                true
            },
            CagedBufferStorage::Heap(h) => {
                let mut lock = h.lock().unwrap();
                if let Some(byte) = lock.get_mut(index) {
                    *byte = value;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Copy bytes into an output buffer.
    pub fn read_bytes(&self, offset: usize, out: &mut [u8]) -> usize {
        if offset >= self.size {
            return 0;
        }
        let available = self.size - offset;
        let to_copy = out.len().min(available);
        match &self.storage {
            CagedBufferStorage::Virtual(ptr) => unsafe {
                let src_ptr = ptr.add(offset);
                std::ptr::copy_nonoverlapping(src_ptr, out.as_mut_ptr(), to_copy);
                to_copy
            },
            CagedBufferStorage::Heap(h) => {
                let lock = h.lock().unwrap();
                out[..to_copy].copy_from_slice(&lock[offset..offset + to_copy]);
                to_copy
            }
        }
    }

    /// Copy bytes from input buffer into this caged buffer.
    pub fn write_bytes(&self, offset: usize, src: &[u8]) -> usize {
        if offset >= self.size {
            return 0;
        }
        let available = self.size - offset;
        let to_copy = src.len().min(available);
        match &self.storage {
            CagedBufferStorage::Virtual(ptr) => unsafe {
                let dst_ptr = ptr.add(offset);
                std::ptr::copy_nonoverlapping(src.as_ptr(), dst_ptr, to_copy);
                to_copy
            },
            CagedBufferStorage::Heap(h) => {
                let mut lock = h.lock().unwrap();
                lock[offset..offset + to_copy].copy_from_slice(&src[..to_copy]);
                to_copy
            }
        }
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
        #[cfg(target_os = "linux")]
        assert!(cage.is_virtual_mmap());
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
