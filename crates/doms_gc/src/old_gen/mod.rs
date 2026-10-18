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

use crate::header::{GC_ALIGN, GcHeader, HEADER_SIZE, TypeTag};
use std::alloc::{Layout, alloc, dealloc};
use std::ptr;

mod marking;
mod space;
mod sweep;

#[cfg(test)]
mod tests;

/// Default old generation size: 8 MB.
const DEFAULT_OLD_GEN_SIZE: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkState {
    Idle,
    MarkingRoots,
    MarkingObjects,
    Complete,
}

/// A free memory chunk in the old generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeChunk {
    /// Byte offset from `start`.
    pub offset: usize,
    /// Size of the chunk in bytes.
    pub size: usize,
}

/// The old generation — tenured object space.
pub struct OldGen {
    /// Start of the old generation memory region.
    pub start: *mut u8,
    /// Size of the region in bytes.
    pub size: usize,
    /// Bump pointer: next free byte.
    pub cursor: *mut u8,
    /// End of the region.
    pub limit: *mut u8,
    /// Number of objects stored.
    pub object_count: usize,
    /// Bytes used.
    pub bytes_used: usize,
    /// Incremental marking state.
    pub mark_state: MarkState,
    /// Worklist for incremental tracing.
    pub mark_worklist: Vec<*mut u8>,
    /// Free chunks reclaimed by sweep for reuse.
    pub free_chunks: Vec<FreeChunk>,
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
