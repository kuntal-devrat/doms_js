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
use crate::trace::{TraceVisitor, TypeRegistry};
use std::alloc::{Layout, alloc, dealloc};
use std::ptr;

mod alloc;
mod collect;
mod inspect;

#[cfg(test)]
mod tests;

/// Default size of each semi-space: 2 MB (fits in CPU L3 cache).
pub const DEFAULT_SEMI_SPACE_SIZE: usize = 2 * 1024 * 1024;

/// Number of scavenges an object must survive before promotion.
pub const PROMOTION_THRESHOLD: u8 = 2;

/// The nursery — a semi-space copying collector for young objects.
///
/// ```text
///   from_space                          to_space
///   ┌──────────────────────────┐       ┌──────────────────────────┐
///   │ [obj1][obj2][obj3]       │       │                          │
///   │ ▲               ▲     ▲  │       │                          │
///   │ start         cursor limit│       │                          │
///   └──────────────────────────┘       └──────────────────────────┘
/// ```
pub struct Nursery {
    /// Pointer to the start of from-space.
    from_start: *mut u8,
    /// Pointer to the start of to-space.
    to_start: *mut u8,
    /// Size of each semi-space in bytes.
    space_size: usize,

    /// Bump pointer: next free byte in from-space.
    cursor: *mut u8,
    /// End of from-space (cursor must not exceed this).
    limit: *mut u8,

    /// Total bytes allocated since last scavenge (before this one).
    pub bytes_allocated: usize,
    /// Number of scavenges performed.
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
        // SAFETY: from_start and to_start were allocated with this layout.
        unsafe {
            dealloc(self.from_start, layout);
            dealloc(self.to_start, layout);
        }
    }
}

// Nursery owns raw pointers but is single-threaded by design.
// Each isolate gets its own nursery — no sharing.
unsafe impl Send for Nursery {}
