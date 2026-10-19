//! Nursery — Semi-space copying collector with bump allocation.
//!
//! The nursery is the young generation of the GC. It uses two equally-sized
//! memory regions (semi-spaces). Objects are allocated in the **from-space**
//! using a bump allocator (extremely fast — just increment a pointer).
//!
//! When the from-space is full, a **scavenge** (minor GC) is triggered:
//! 1. Walk all roots and copy live objects to **to-space** using Cheney's algorithm.
//! 2. Update all references to point to the new locations.
//! 3. Swap from-space and to-space.
//!
//! Objects that survive multiple scavenges are **promoted** to the old generation.
//!
//! ## Performance Characteristics
//!
//! - **Allocation:** O(1) bump pointer — as fast as stack allocation.
//! - **Collection:** O(live objects) — dead objects cost nothing.
//! - **Pause time:** Proportional to nursery size and survival rate.

use crate::header::{GC_ALIGN, GcHeader, HEADER_SIZE, TypeTag};
use crate::old_gen::FreeChunk;
use crate::trace::{TraceVisitor, TypeRegistry};
use std::alloc::{Layout, alloc, dealloc};
use std::ptr;

mod alloc;
mod collect;
mod inspect;

#[cfg(test)]
mod tests;

/// Default size of the nursery arena: 2 MB (fits in CPU L3 cache).
pub const DEFAULT_NURSERY_SIZE: usize = 2 * 1024 * 1024;
pub const DEFAULT_SEMI_SPACE_SIZE: usize = DEFAULT_NURSERY_SIZE;

/// Number of collections an object survives before promotion.
pub const PROMOTION_THRESHOLD: u8 = 2;

/// The nursery — an immovable bump-allocated arena for young objects.
/// Objects are allocated via fast bump pointer, never moved, and swept in-place.
pub struct Nursery {
    /// Pointer to the start of nursery space.
    pub(crate) from_start: *mut u8,
    /// Size of nursery space in bytes.
    pub(crate) space_size: usize,

    /// Bump pointer: next free byte in nursery space.
    pub(crate) cursor: *mut u8,
    /// End of nursery space (cursor must not exceed this).
    pub(crate) limit: *mut u8,

    /// Free memory chunks reclaimed by in-place sweep for reuse.
    pub free_chunks: Vec<FreeChunk>,

    /// Total bytes currently alive or allocated.
    pub bytes_allocated: usize,
    /// Number of collections performed.
    pub scavenge_count: u64,
    /// Total objects allocated.
    pub total_allocations: u64,
    /// Registry mapping TypeTags to interior trace visitors.
    pub type_registry: TypeRegistry,
}

impl Drop for Nursery {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.space_size, GC_ALIGN)
            .expect("Invalid layout during dealloc");
        // SAFETY: from_start was allocated with this layout.
        unsafe {
            dealloc(self.from_start, layout);
        }
    }
}

// Nursery owns raw pointers but is single-threaded by design.
// Each isolate gets its own nursery — no sharing.
unsafe impl Send for Nursery {}
