//! Old Generation — Mark-sweep collector for long-lived objects.
//!
//! Objects that survive multiple nursery scavenges are promoted here.
//! The old generation uses a mark-sweep-compact strategy:
//!
//! 1. **Mark:** Walk from roots, mark all reachable objects.
//! 2. **Sweep:** Free unmarked objects, returning memory to free lists.
//! 3. **Compact:** (Phase 3) Move objects to reduce fragmentation.
//!
//! ## Current Status
//!
//! This is a **Phase 0 stub**. The full mark-sweep-compact implementation
//! will arrive in Phase 1. For now, promoted objects are allocated in a
//! simple Vec-backed region with no collection.

use crate::header::{GC_ALIGN, GcHeader, HEADER_SIZE};
use std::alloc::{Layout, alloc, dealloc};
use std::ptr;

/// Default old generation size: 64 MB.
const DEFAULT_OLD_GEN_SIZE: usize = 64 * 1024 * 1024;

/// The old generation — tenured object space.
///
/// Currently a simple bump allocator (Phase 0). Will be upgraded to
/// mark-sweep with free lists in Phase 1.
pub struct OldGen {
    /// Start of the old generation memory region.
    start: *mut u8,
    /// Size of the region in bytes.
    size: usize,
    /// Bump pointer: next free byte.
    cursor: *mut u8,
    /// End of the region.
    limit: *mut u8,
    /// Number of objects stored.
    pub object_count: usize,
    /// Bytes used.
    pub bytes_used: usize,
}

impl OldGen {
    /// Create a new old generation with the given size.
    pub fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, GC_ALIGN).expect("Invalid layout for old gen");

        // SAFETY: Layout is valid (checked above).
        let start = unsafe { alloc(layout) };
        if start.is_null() {
            panic!("Failed to allocate old generation ({} bytes)", size);
        }

        // SAFETY: start is valid for `size` bytes.
        unsafe {
            ptr::write_bytes(start, 0, size);
        }

        let limit = unsafe { start.add(size) };

        Self { start, size, cursor: start, limit, object_count: 0, bytes_used: 0 }
    }

    /// Create an old generation with the default size (64 MB).
    pub fn with_default_size() -> Self {
        Self::new(DEFAULT_OLD_GEN_SIZE)
    }

    /// Promote an object from the nursery to the old generation.
    ///
    /// Copies the object (header + payload) into old gen space.
    /// Returns a pointer to the new payload location, or `None` if full.
    ///
    /// # Safety
    /// `payload_ptr` must point to a valid nursery object.
    pub unsafe fn promote(&mut self, payload_ptr: *const u8) -> Option<*mut u8> {
        // SAFETY: payload_ptr is a valid GC allocation in the nursery.
        let header = unsafe { &*((payload_ptr as *const GcHeader).sub(1)) };
        let total_size = header.total_size();

        // Check space.
        let remaining = unsafe { self.limit.offset_from(self.cursor) } as usize;
        if total_size > remaining {
            return None;
        }

        // Copy header + payload.
        let src = unsafe { payload_ptr.sub(HEADER_SIZE) };
        // SAFETY: Non-overlapping copy, both regions are valid.
        unsafe {
            ptr::copy_nonoverlapping(src, self.cursor, total_size);
        }

        // Mark the new copy as tenured.
        let new_header = self.cursor as *mut GcHeader;
        // SAFETY: We just copied a valid header here.
        unsafe {
            (*new_header).set_tenured();
        }

        let new_payload = unsafe { self.cursor.add(HEADER_SIZE) };
        self.cursor = unsafe { self.cursor.add(total_size) };
        self.object_count += 1;
        self.bytes_used += total_size;

        Some(new_payload)
    }

    /// Check if a pointer falls within the old generation.
    #[inline]
    pub fn contains(&self, ptr: *const u8) -> bool {
        let addr = ptr as usize;
        let start = self.start as usize;
        addr >= start && addr < start + self.size
    }

    /// Returns the utilization ratio.
    pub fn utilization(&self) -> f64 {
        self.bytes_used as f64 / self.size as f64
    }

    // TODO: Phase 1 — mark(), sweep(), compact()
}

impl Drop for OldGen {
    fn drop(&mut self) {
        let layout =
            Layout::from_size_align(self.size, GC_ALIGN).expect("Invalid layout during dealloc");
        // SAFETY: start was allocated with this layout.
        unsafe {
            dealloc(self.start, layout);
        }
    }
}

unsafe impl Send for OldGen {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::TypeTag;
    use crate::nursery::Nursery;

    #[test]
    fn test_old_gen_creation() {
        let old_gen = OldGen::new(4096);
        assert_eq!(old_gen.object_count, 0);
        assert_eq!(old_gen.bytes_used, 0);
    }

    #[test]
    fn test_promote_from_nursery() {
        let mut nursery = Nursery::new(4096);
        let mut old_gen = OldGen::new(4096);

        // Allocate in nursery.
        let ptr = nursery.allocate(16, TypeTag::Object).unwrap();
        unsafe {
            *(ptr as *mut u64) = 0xDEAD_BEEF;
        }

        // Promote to old gen.
        let new_ptr = unsafe { old_gen.promote(ptr) };
        assert!(new_ptr.is_some());
        let new_ptr = new_ptr.unwrap();

        assert!(old_gen.contains(new_ptr));
        assert_eq!(old_gen.object_count, 1);

        // Data should be preserved.
        unsafe {
            assert_eq!(*(new_ptr as *const u64), 0xDEAD_BEEF);
        }
    }
}
