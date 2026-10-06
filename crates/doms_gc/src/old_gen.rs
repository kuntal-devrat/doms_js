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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkState {
    Idle,
    MarkingRoots,
    MarkingObjects,
    Complete,
}

/// The old generation — tenured object space.
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
    /// Incremental marking state.
    pub mark_state: MarkState,
    /// Worklist for incremental tracing.
    pub mark_worklist: Vec<*mut u8>,
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

        Self {
            start,
            size,
            cursor: start,
            limit,
            object_count: 0,
            bytes_used: 0,
            mark_state: MarkState::Idle,
            mark_worklist: Vec::new(),
        }
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

    /// Begin an incremental marking phase from the given root pointers.
    pub fn start_incremental_marking(&mut self, roots: &[*mut u8]) {
        self.mark_state = MarkState::MarkingObjects;
        self.mark_worklist.clear();
        for &root in roots {
            if !root.is_null() && self.contains(root) {
                unsafe {
                    let header = &mut *((root as *mut GcHeader).sub(1));
                    if !header.is_marked() {
                        header.set_marked();
                        self.mark_worklist.push(root);
                    }
                }
            }
        }
        if self.mark_worklist.is_empty() {
            self.mark_state = MarkState::Complete;
        }
    }

    /// Perform a single step of incremental marking, processing at most `work_limit` objects.
    /// Returns `true` if marking is complete, `false` if more work remains.
    pub fn mark_step(&mut self, work_limit: usize) -> bool {
        if self.mark_state != MarkState::MarkingObjects {
            return self.mark_state == MarkState::Complete;
        }

        let mut work_done = 0;
        while let Some(_ptr) = self.mark_worklist.pop() {
            work_done += 1;
            if self.mark_worklist.is_empty() {
                self.mark_state = MarkState::Complete;
                return true;
            }
            if work_done >= work_limit {
                return false;
            }
        }

        self.mark_state = MarkState::Complete;
        true
    }

    /// Reset all mark bits and the marking state back to idle.
    pub fn reset_marks(&mut self) {
        let mut curr = self.start;
        while curr < self.cursor {
            unsafe {
                let header = &mut *(curr as *mut GcHeader);
                header.clear_marked();
                let sz = header.total_size();
                if sz == 0 {
                    break;
                }
                curr = curr.add(sz);
            }
        }
        self.mark_state = MarkState::Idle;
        self.mark_worklist.clear();
    }

    /// Check if an object in the old generation is marked.
    pub fn is_marked(&self, payload_ptr: *const u8) -> bool {
        if payload_ptr.is_null() || !self.contains(payload_ptr) {
            return false;
        }
        unsafe {
            let header = &*((payload_ptr as *const GcHeader).sub(1));
            header.is_marked()
        }
    }
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

    #[test]
    fn test_incremental_marking() {
        let mut nursery = Nursery::new(4096);
        let mut old_gen = OldGen::new(4096);

        // Allocate two objects and promote them
        let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
        let p2 = nursery.allocate(16, TypeTag::Object).unwrap();

        let old_p1 = unsafe { old_gen.promote(p1).unwrap() };
        let old_p2 = unsafe { old_gen.promote(p2).unwrap() };

        assert!(!old_gen.is_marked(old_p1));
        assert!(!old_gen.is_marked(old_p2));
        assert_eq!(old_gen.mark_state, MarkState::Idle);

        // Start incremental marking with old_p1 and old_p2 as roots
        let roots = vec![old_p1, old_p2];
        old_gen.start_incremental_marking(&roots);

        assert!(old_gen.is_marked(old_p1));
        assert!(old_gen.is_marked(old_p2));
        assert_eq!(old_gen.mark_state, MarkState::MarkingObjects);
        assert_eq!(old_gen.mark_worklist.len(), 2);

        // Step 1: work limit 1 -> should process 1 object, not finished yet
        let finished = old_gen.mark_step(1);
        assert!(!finished);
        assert_eq!(old_gen.mark_state, MarkState::MarkingObjects);
        assert_eq!(old_gen.mark_worklist.len(), 1);

        // Step 2: work limit 1 -> should process the remaining object and finish
        let finished2 = old_gen.mark_step(1);
        assert!(finished2);
        assert_eq!(old_gen.mark_state, MarkState::Complete);
        assert!(old_gen.mark_worklist.is_empty());

        // Reset marks
        old_gen.reset_marks();
        assert_eq!(old_gen.mark_state, MarkState::Idle);
        assert!(!old_gen.is_marked(old_p1));
        assert!(!old_gen.is_marked(old_p2));
    }
}

