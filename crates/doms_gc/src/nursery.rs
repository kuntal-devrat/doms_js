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

/// Default size of each semi-space: 4 MB.
pub const DEFAULT_SEMI_SPACE_SIZE: usize = 4 * 1024 * 1024;

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

impl Nursery {
    /// Create a new nursery with the given semi-space size.
    ///
    /// Allocates two contiguous memory regions of `space_size` bytes each.
    ///
    /// # Panics
    /// Panics if memory allocation fails.
    pub fn new(space_size: usize) -> Self {
        assert!(space_size >= 4096, "Semi-space size must be at least 4KB");
        assert!(
            space_size.is_power_of_two() || space_size % 4096 == 0,
            "Semi-space size should be page-aligned"
        );

        let layout =
            Layout::from_size_align(space_size, GC_ALIGN).expect("Invalid layout for semi-space");

        // SAFETY: We allocate two independent memory regions. Layout is valid
        // (checked above). We check for null.
        let from_start = unsafe { alloc(layout) };
        let to_start = unsafe { alloc(layout) };

        if from_start.is_null() || to_start.is_null() {
            panic!("Failed to allocate nursery semi-spaces ({} bytes each)", space_size);
        }

        // Zero-initialize both spaces for safety.
        // SAFETY: Both pointers are valid for space_size bytes.
        unsafe {
            ptr::write_bytes(from_start, 0, space_size);
            ptr::write_bytes(to_start, 0, space_size);
        }

        let limit = unsafe { from_start.add(space_size) };

        Self {
            from_start,
            to_start,
            space_size,
            cursor: from_start,
            limit,
            bytes_allocated: 0,
            scavenge_count: 0,
            total_allocations: 0,
            type_registry: TypeRegistry::new(),
        }
    }

    /// Register a trace visitor for interior reference tracing during scavenge.
    pub fn register_tracer(&mut self, tag: TypeTag, visitor: TraceVisitor) {
        self.type_registry.register(tag, visitor);
    }

    /// Create a nursery with the default semi-space size (4 MB).
    pub fn with_default_size() -> Self {
        Self::new(DEFAULT_SEMI_SPACE_SIZE)
    }

    /// Allocate `payload_size` bytes in the nursery.
    ///
    /// Returns a pointer to the payload (after the GcHeader) or `None` if
    /// the nursery is full and a scavenge is needed.
    ///
    /// The allocation is aligned to [`GC_ALIGN`] bytes.
    ///
    /// # Arguments
    /// - `payload_size`: Size of the object payload in bytes (excluding header).
    /// - `type_tag`: Type tag for the object (used during GC tracing).
    pub fn allocate(&mut self, payload_size: usize, type_tag: TypeTag) -> Option<*mut u8> {
        let total_size = Self::align_up(HEADER_SIZE + payload_size, GC_ALIGN);

        // Check if we have space.
        // SAFETY: cursor and limit are always valid pointers within from-space.
        let remaining = unsafe { self.limit.offset_from(self.cursor) } as usize;
        if total_size > remaining {
            return None; // Nursery full — caller should trigger scavenge.
        }

        let header_ptr = self.cursor as *mut GcHeader;

        // Write the object header.
        // SAFETY: cursor points to valid, writable memory within from-space.
        // We've verified there's enough space.
        unsafe {
            ptr::write(header_ptr, GcHeader::new(total_size as u32, type_tag));
        }

        // The payload starts immediately after the header.
        // SAFETY: header_ptr + 1 is within the allocation we just verified.
        let payload_ptr = unsafe { header_ptr.add(1) as *mut u8 };

        // Zero-initialize the payload.
        // SAFETY: payload_ptr is valid for payload_size bytes.
        unsafe {
            ptr::write_bytes(payload_ptr, 0, total_size - HEADER_SIZE);
        }

        // Bump the cursor.
        // SAFETY: total_size was verified to fit within remaining space.
        self.cursor = unsafe { self.cursor.add(total_size) };
        self.bytes_allocated += total_size;
        self.total_allocations += 1;

        Some(payload_ptr)
    }

    /// Allocate and write a value of type `T` into the nursery.
    ///
    /// Returns a pointer to the written value, or `None` if the nursery is full.
    ///
    /// # Safety
    /// - `T` must be safe to store on the GC heap (no non-`'static` references).
    /// - The caller must ensure `T` outlives the GC or is properly traced.
    pub unsafe fn allocate_value<T>(&mut self, value: T, type_tag: TypeTag) -> Option<*mut T> {
        let payload_size = std::mem::size_of::<T>();
        let payload_ptr = self.allocate(payload_size, type_tag)?;

        // SAFETY: payload_ptr points to zeroed, properly aligned memory
        // within the nursery. We have exclusive access.
        unsafe {
            ptr::write(payload_ptr as *mut T, value);
        }

        Some(payload_ptr as *mut T)
    }

    /// Check if a pointer falls within the from-space.
    #[inline]
    pub fn contains(&self, ptr: *const u8) -> bool {
        let addr = ptr as usize;
        let start = self.from_start as usize;
        let end = start + self.space_size;
        addr >= start && addr < end
    }

    /// Check if a pointer falls within the to-space.
    #[inline]
    pub fn to_space_contains(&self, ptr: *const u8) -> bool {
        let addr = ptr as usize;
        let start = self.to_start as usize;
        let end = start + self.space_size;
        addr >= start && addr < end
    }

    /// Returns the number of bytes currently used in from-space.
    #[inline]
    pub fn used_bytes(&self) -> usize {
        // SAFETY: cursor and from_start are always valid pointers.
        unsafe { self.cursor.offset_from(self.from_start) as usize }
    }

    /// Returns the number of bytes remaining in from-space.
    #[inline]
    pub fn remaining_bytes(&self) -> usize {
        self.space_size - self.used_bytes()
    }

    /// Returns the semi-space size.
    #[inline]
    pub fn space_size(&self) -> usize {
        self.space_size
    }

    /// Returns the utilization ratio (0.0 to 1.0).
    #[inline]
    pub fn utilization(&self) -> f64 {
        self.used_bytes() as f64 / self.space_size as f64
    }

    /// Perform a scavenge (minor GC).
    ///
    /// Copies all live objects from from-space to to-space using Cheney's
    /// algorithm, then swaps the spaces.
    ///
    /// # Arguments
    /// - `roots`: Mutable slice of root pointers. Each root points to the
    ///   payload of a GC object. Roots are updated in-place to point to
    ///   the new locations in to-space.
    ///
    /// # Returns
    /// The number of bytes that survived (were copied to to-space).
    pub fn scavenge(&mut self, roots: &mut [*mut u8]) -> usize {
        // Reset to-space cursor.
        let mut to_cursor = self.to_start;
        let to_limit = unsafe { self.to_start.add(self.space_size) };

        // Phase 1: Copy all root-referenced objects to to-space.
        for root in roots.iter_mut() {
            if root.is_null() {
                continue;
            }
            if self.contains(*root) {
                *root = self.copy_object(*root, &mut to_cursor, to_limit);
            }
        }

        // Phase 2: Cheney scan — walk the copied objects in to-space and
        // copy their referenced objects too (breadth-first).
        let mut scan = self.to_start;
        while (scan as usize) < (to_cursor as usize) {
            // Read the header of the next object to scan.
            // SAFETY: scan points to a valid GcHeader in to-space.
            let header = unsafe { &*(scan as *const GcHeader) };
            let total_size = header.total_size();
            let tag = header.tag();

            let payload = unsafe { scan.add(HEADER_SIZE) };
            unsafe {
                self.type_registry.trace(tag, payload, &mut |ref_slot: *mut *mut u8| {
                    if !ref_slot.is_null() {
                        let child_ptr = *ref_slot;
                        if !child_ptr.is_null() && self.contains(child_ptr) {
                            *ref_slot = self.copy_object(child_ptr, &mut to_cursor, to_limit);
                        }
                    }
                });
            }

            // Advance scan to the next object.
            // SAFETY: total_size is the aligned size written during allocation.
            scan = unsafe { scan.add(total_size) };
        }

        // Phase 3: Swap from-space and to-space.
        std::mem::swap(&mut self.from_start, &mut self.to_start);
        self.cursor = to_cursor;
        self.limit = unsafe { self.from_start.add(self.space_size) };

        // Clear the old from-space (now to-space) for debugging.
        // SAFETY: to_start (the old from-space) is valid for space_size bytes.
        unsafe {
            ptr::write_bytes(self.to_start, 0xDD, self.space_size);
        }

        self.scavenge_count += 1;
        let survived = unsafe { to_cursor.offset_from(self.from_start) } as usize;
        self.bytes_allocated = survived;

        log::debug!(
            "Scavenge #{}: {} bytes survived ({:.1}% survival rate)",
            self.scavenge_count,
            survived,
            if self.space_size > 0 {
                survived as f64 / self.space_size as f64 * 100.0
            } else {
                0.0
            }
        );

        survived
    }

    /// Copy a single object from from-space to to-space.
    ///
    /// If the object has already been forwarded, returns the forwarding address.
    /// Otherwise, copies the object and installs a forwarding pointer.
    ///
    /// # Arguments
    /// - `payload_ptr`: Pointer to the object's payload in from-space.
    /// - `to_cursor`: Mutable reference to the to-space allocation cursor.
    /// - `to_limit`: End of to-space.
    ///
    /// # Returns
    /// Pointer to the object's payload in to-space.
    fn copy_object(
        &self,
        payload_ptr: *mut u8,
        to_cursor: &mut *mut u8,
        to_limit: *const u8,
    ) -> *mut u8 {
        // Get the header (immediately before the payload).
        // SAFETY: payload_ptr points to a valid GC allocation in from-space.
        let header_ptr = unsafe { (payload_ptr as *mut GcHeader).sub(1) };
        let header = unsafe { &mut *header_ptr };

        // If already forwarded, return the forwarding address.
        if header.is_forwarded() {
            // SAFETY: Forwarding address was set by a previous copy_object call.
            return unsafe { header.forwarding_address() };
        }

        let total_size = header.total_size();

        // Check to-space has room.
        let space_remaining = unsafe { to_limit.offset_from(*to_cursor) } as usize;
        assert!(
            total_size <= space_remaining,
            "To-space overflow during scavenge! Need {} bytes, have {}",
            total_size,
            space_remaining
        );

        // Copy the entire object (header + payload) to to-space.
        let new_header_ptr = *to_cursor;
        // SAFETY: Both pointers are valid, non-overlapping, and to-space has space.
        unsafe {
            ptr::copy_nonoverlapping(header_ptr as *const u8, new_header_ptr, total_size);
        }

        // The new payload is at header + HEADER_SIZE.
        // SAFETY: new_header_ptr + HEADER_SIZE is within to-space.
        let new_payload = unsafe { new_header_ptr.add(HEADER_SIZE) };

        // Install forwarding pointer in the old from-space object.
        // SAFETY: header_ptr is in from-space and still writable.
        unsafe {
            header.set_forwarding_address(new_payload);
        }

        // Bump to-space cursor.
        *to_cursor = unsafe { (*to_cursor).add(total_size) };

        new_payload
    }

    /// Reset the nursery, discarding all objects.
    ///
    /// This is useful for testing or when all objects have been promoted.
    pub fn reset(&mut self) {
        self.cursor = self.from_start;
        self.bytes_allocated = 0;
        // SAFETY: from_start is valid for space_size bytes.
        unsafe {
            ptr::write_bytes(self.from_start, 0, self.space_size);
        }
    }

    /// Iterate over all live objects in from-space.
    ///
    /// Calls `callback` with the header and payload pointer for each object.
    pub fn for_each_object(&self, mut callback: impl FnMut(&GcHeader, *const u8)) {
        let mut scan = self.from_start;
        while (scan as usize) < (self.cursor as usize) {
            // SAFETY: scan points to a valid GcHeader.
            let header = unsafe { &*(scan as *const GcHeader) };
            let payload = unsafe { scan.add(HEADER_SIZE) };
            callback(header, payload);
            // SAFETY: total_size is the aligned size.
            scan = unsafe { scan.add(header.total_size()) };
        }
    }

    /// Count the number of live objects in from-space.
    pub fn object_count(&self) -> usize {
        let mut count = 0;
        self.for_each_object(|_, _| count += 1);
        count
    }

    /// Round up `size` to the nearest multiple of `align`.
    #[inline]
    const fn align_up(size: usize, align: usize) -> usize {
        (size + align - 1) & !(align - 1)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SPACE_SIZE: usize = 4096; // 4 KB for tests

    #[test]
    fn test_nursery_creation() {
        let nursery = Nursery::new(TEST_SPACE_SIZE);
        assert_eq!(nursery.space_size(), TEST_SPACE_SIZE);
        assert_eq!(nursery.used_bytes(), 0);
        assert_eq!(nursery.remaining_bytes(), TEST_SPACE_SIZE);
        assert_eq!(nursery.object_count(), 0);
    }

    #[test]
    fn test_bump_allocation() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        // Allocate a 24-byte payload.
        let ptr = nursery.allocate(24, TypeTag::Object);
        assert!(ptr.is_some());

        let ptr = ptr.unwrap();
        assert!(!ptr.is_null());
        assert!(nursery.contains(ptr));

        // Used bytes should be header (8) + payload (24) = 32.
        assert_eq!(nursery.used_bytes(), 32);
        assert_eq!(nursery.object_count(), 1);
    }

    #[test]
    fn test_multiple_allocations() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
        let p2 = nursery.allocate(32, TypeTag::String).unwrap();
        let p3 = nursery.allocate(8, TypeTag::Array).unwrap();

        // All pointers should be different and in from-space.
        assert_ne!(p1, p2);
        assert_ne!(p2, p3);
        assert!(nursery.contains(p1));
        assert!(nursery.contains(p2));
        assert!(nursery.contains(p3));
        assert_eq!(nursery.object_count(), 3);

        // Verify headers.
        unsafe {
            let h1 = &*((p1 as *const GcHeader).sub(1));
            assert_eq!(h1.tag(), TypeTag::Object);

            let h2 = &*((p2 as *const GcHeader).sub(1));
            assert_eq!(h2.tag(), TypeTag::String);
        }
    }

    #[test]
    fn test_allocation_fills_nursery() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        // Each allocation: 8 (header) + 8 (payload, aligned) = 16 bytes.
        // 4096 / 16 = 256 allocations should fit.
        let mut count = 0;
        while nursery.allocate(8, TypeTag::Raw).is_some() {
            count += 1;
        }
        assert_eq!(count, 256);

        // Next allocation should fail.
        assert!(nursery.allocate(8, TypeTag::Raw).is_none());
    }

    #[test]
    fn test_allocate_value() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        let ptr = unsafe { nursery.allocate_value(42u64, TypeTag::Raw) };
        assert!(ptr.is_some());

        let ptr = ptr.unwrap();
        assert_eq!(unsafe { *ptr }, 42u64);
    }

    #[test]
    fn test_scavenge_with_roots() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        // Allocate 3 objects.
        let p1 = nursery.allocate(16, TypeTag::Object).unwrap();
        let _p2 = nursery.allocate(16, TypeTag::String).unwrap(); // Not rooted — should die.
        let p3 = nursery.allocate(16, TypeTag::Array).unwrap();

        // Write some data so we can verify it survives.
        unsafe {
            *(p1 as *mut u64) = 0xDEAD_BEEF;
            *(p3 as *mut u64) = 0xCAFE_BABE;
        }

        // Only root p1 and p3.
        let mut roots = vec![p1, p3];

        let survived = nursery.scavenge(&mut roots);

        // Two objects survived, each 24 bytes (8 header + 16 payload).
        assert_eq!(survived, 48);
        assert_eq!(nursery.object_count(), 2);

        // Roots should be updated to point to new locations.
        let new_p1 = roots[0];
        let new_p3 = roots[1];

        assert!(nursery.contains(new_p1));
        assert!(nursery.contains(new_p3));
        assert_ne!(new_p1, p1, "Scavenge should have moved the object");

        // Data should be preserved.
        unsafe {
            assert_eq!(*(new_p1 as *const u64), 0xDEAD_BEEF);
            assert_eq!(*(new_p3 as *const u64), 0xCAFE_BABE);
        }
    }

    #[test]
    fn test_scavenge_forwarding() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        let p1 = nursery.allocate(16, TypeTag::Object).unwrap();

        // If the same object is in the root set twice, forwarding should
        // ensure both roots point to the same new location.
        let mut roots = vec![p1, p1];
        nursery.scavenge(&mut roots);

        assert_eq!(roots[0], roots[1], "Both roots should be forwarded to the same location");
    }

    #[test]
    fn test_scavenge_empty_roots() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        // Allocate some objects.
        nursery.allocate(16, TypeTag::Object).unwrap();
        nursery.allocate(16, TypeTag::String).unwrap();

        // No roots — everything should be collected.
        let mut roots: Vec<*mut u8> = vec![];
        let survived = nursery.scavenge(&mut roots);

        assert_eq!(survived, 0);
        assert_eq!(nursery.object_count(), 0);
    }

    #[test]
    fn test_reset() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        nursery.allocate(16, TypeTag::Object).unwrap();
        nursery.allocate(16, TypeTag::String).unwrap();
        assert_eq!(nursery.object_count(), 2);

        nursery.reset();
        assert_eq!(nursery.object_count(), 0);
        assert_eq!(nursery.used_bytes(), 0);
    }

    #[test]
    fn test_utilization() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        assert_eq!(nursery.utilization(), 0.0);

        // Fill half the nursery.
        let alloc_size = TEST_SPACE_SIZE / 2;
        nursery.allocate(alloc_size - HEADER_SIZE, TypeTag::Raw).unwrap();

        let util = nursery.utilization();
        assert!((util - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_contains() {
        let nursery = Nursery::new(TEST_SPACE_SIZE);
        let outside: u8 = 0;
        assert!(!nursery.contains(&outside as *const u8));
    }

    #[test]
    fn test_scavenge_with_interior_references() {
        let mut nursery = Nursery::new(TEST_SPACE_SIZE);

        // Register a tracer for TypeTag::Object that treats the first 8 bytes as a pointer slot.
        unsafe fn trace_first_slot(payload: *mut u8, visitor: &mut dyn FnMut(*mut *mut u8)) {
            let slot = payload as *mut *mut u8;
            visitor(slot);
        }
        nursery.register_tracer(TypeTag::Object, trace_first_slot);

        // Allocate child object B.
        let child_ptr = nursery.allocate(16, TypeTag::Raw).unwrap();
        unsafe {
            *(child_ptr as *mut u64) = 0xCAFEBABE;
        }

        // Allocate parent object A, containing a pointer to child B.
        let parent_ptr = nursery.allocate(16, TypeTag::Object).unwrap();
        unsafe {
            *(parent_ptr as *mut *mut u8) = child_ptr;
        }

        // Root ONLY parent A. Child B is NOT directly rooted.
        let mut roots = vec![parent_ptr];
        nursery.scavenge(&mut roots);

        let new_parent_ptr = roots[0];
        assert!(nursery.contains(new_parent_ptr));
        assert_ne!(new_parent_ptr, parent_ptr); // moved to to-space

        // Verify child was also moved to to-space via Cheney interior scan!
        let new_child_ptr = unsafe { *(new_parent_ptr as *const *mut u8) };
        assert!(nursery.contains(new_child_ptr));
        assert_ne!(new_child_ptr, child_ptr); // child moved

        // Verify child payload is intact
        let child_val = unsafe { *(new_child_ptr as *const u64) };
        assert_eq!(child_val, 0xCAFEBABE);
    }
}
