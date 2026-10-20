//! Top-level Heap — manages all GC memory spaces.
//!
//! The [`Heap`] is the primary interface for allocating GC-managed objects.
//! It routes allocations to the appropriate space (nursery, old gen, or
//! large object space) and triggers collections when needed.

use crate::header::TypeTag;
use crate::large_object::{LARGE_OBJECT_THRESHOLD, LargeObjectSpace};
use crate::nursery::Nursery;
use crate::old_gen::OldGen;

/// Configuration for a GC heap.
#[derive(Debug, Clone)]
pub struct HeapConfig {
    /// Size of each nursery semi-space in bytes.
    pub nursery_size: usize,
    /// Size of the old generation in bytes.
    pub old_gen_size: usize,
}

impl Default for HeapConfig {
    fn default() -> Self {
        Self {
            nursery_size: 2 * 1024 * 1024, // 2 MB (fits in CPU cache)
            old_gen_size: 8 * 1024 * 1024, // 8 MB
        }
    }
}

/// Statistics about the heap's current state.
#[derive(Debug, Clone)]
pub struct HeapStats {
    /// Nursery utilization (0.0 - 1.0).
    pub nursery_utilization: f64,
    /// Old gen utilization (0.0 - 1.0).
    pub old_gen_utilization: f64,
    /// Number of nursery scavenges performed.
    pub scavenge_count: u64,
    /// Total allocations across all spaces.
    pub total_allocations: u64,
    /// Number of large objects.
    pub large_object_count: usize,
}

/// The remembered set tracks references from older generation or large objects
/// to objects in the nursery. During minor collections (scavenge), remembered slots
/// are traced as roots so young objects referenced by tenured objects are preserved.
#[derive(Debug, Default)]
pub struct RememberedSet {
    /// Addresses of pointer slots (*mut *mut u8) holding references into the nursery.
    pub slots: std::collections::HashSet<usize>,
}

impl RememberedSet {
    pub fn new() -> Self {
        Self { slots: std::collections::HashSet::new() }
    }

    /// Add a pointer slot to the remembered set.
    #[inline]
    pub fn remember(&mut self, slot: *mut *mut u8) {
        if !slot.is_null() {
            self.slots.insert(slot as usize);
        }
    }

    /// Clear all remembered slots.
    #[inline]
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    /// Number of remembered slots.
    #[inline]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// The GC Heap — owns all memory spaces and coordinates collection.
///
/// # Usage
///
/// ```rust
/// use doms_gc::{Heap, heap::HeapConfig};
/// use doms_gc::header::TypeTag;
///
/// let mut heap = Heap::new(HeapConfig::default());
///
/// // Allocate a 32-byte object.
/// let ptr = heap.allocate(32, TypeTag::Object);
/// assert!(ptr.is_some());
/// ```
pub struct Heap {
    /// Young generation: fast bump allocation, semi-space copying.
    pub nursery: Nursery,
    /// Old generation: long-lived objects.
    pub old_gen: OldGen,
    /// Large object space: objects > 8 KB.
    pub large_objects: LargeObjectSpace,
    /// Root set: pointers the GC must trace from.
    /// In a real engine, these come from the VM stack, global object, etc.
    roots: Vec<*mut u8>,
    /// Remembered set for generational write barrier tracking.
    pub remembered_set: RememberedSet,
}

impl Heap {
    /// Create a new heap with the given configuration.
    pub fn new(config: HeapConfig) -> Self {
        Self {
            nursery: Nursery::new(config.nursery_size),
            old_gen: OldGen::new(config.old_gen_size),
            large_objects: LargeObjectSpace::new(),
            roots: Vec::new(),
            remembered_set: RememberedSet::new(),
        }
    }

    /// Create a heap with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(HeapConfig::default())
    }

    /// Allocate an object of `payload_size` bytes.
    ///
    /// Small objects go to the nursery. Large objects (> 8 KB) go directly
    /// to the large object space. If the nursery is full, a scavenge is
    /// triggered automatically.
    ///
    /// Returns a pointer to the payload, or `None` on OOM.
    pub fn allocate(&mut self, payload_size: usize, type_tag: TypeTag) -> Option<*mut u8> {
        // Large objects bypass the nursery.
        if payload_size >= LARGE_OBJECT_THRESHOLD {
            return self.large_objects.allocate(payload_size, type_tag);
        }

        // Try nursery allocation.
        if let Some(ptr) = self.nursery.allocate(payload_size, type_tag) {
            return Some(ptr);
        }

        // Nursery full — trigger scavenge.
        self.collect_nursery();

        // Retry allocation after scavenge.
        if let Some(ptr) = self.nursery.allocate(payload_size, type_tag) {
            return Some(ptr);
        }

        // If nursery is still full, try old generation.
        if let Some(ptr) = self.old_gen.allocate(payload_size, type_tag) {
            return Some(ptr);
        }

        // Fallback to large object space.
        self.large_objects.allocate(payload_size, type_tag)
    }

    /// Allocate and write a value of type `T` into the GC heap.
    ///
    /// # Safety
    /// - `T` must be safe to store on the GC heap.
    pub unsafe fn allocate_value<T>(&mut self, value: T, type_tag: TypeTag) -> Option<*mut T> {
        let payload_size = std::mem::size_of::<T>();
        let payload_ptr = self.allocate(payload_size, type_tag)?;
        unsafe {
            std::ptr::write(payload_ptr as *mut T, value);
        }
        Some(payload_ptr as *mut T)
    }

    /// Register a root pointer. The GC will trace from this pointer
    /// during collections.
    pub fn add_root(&mut self, root: *mut u8) {
        self.roots.push(root);
    }

    /// Remove a root pointer.
    pub fn remove_root(&mut self, root: *mut u8) {
        self.roots.retain(|r| *r != root);
    }

    /// Register a type tracer for interior reference tracing in the nursery.
    pub fn register_tracer(&mut self, tag: TypeTag, visitor: crate::trace::TraceVisitor) {
        self.nursery.register_tracer(tag, visitor);
    }

    /// Post-write barrier: records when an old generation or large object
    /// is updated to point to a nursery (young generation) object.
    ///
    /// # Safety
    /// `slot` must be a valid, readable pointer to a pointer.
    pub unsafe fn write_barrier(&mut self, slot: *mut *mut u8) {
        if slot.is_null() {
            return;
        }
        let target = unsafe { *slot };
        if !target.is_null() && self.nursery.contains(target) {
            self.remembered_set.remember(slot);
        }
    }

    /// Trigger a nursery scavenge (minor GC).
    pub fn collect_nursery(&mut self) {
        let mut all_roots = self.roots.clone();
        for &slot_addr in &self.remembered_set.slots {
            let slot = slot_addr as *mut *mut u8;
            let target = unsafe { *slot };
            if !target.is_null() && self.nursery.contains(target) {
                all_roots.push(target);
            }
        }

        self.nursery.scavenge(&mut all_roots);

        let nursery = &self.nursery;
        self.remembered_set.slots.retain(|&slot_addr| {
            let slot = slot_addr as *mut *mut u8;
            let target = unsafe { *slot };
            !target.is_null() && nursery.contains(target)
        });
    }

    /// Trigger an old generation mark-sweep collection (non-moving, major GC).
    pub fn collect_old_gen_sweep(&mut self) -> usize {
        self.old_gen.start_incremental_marking(&self.roots);
        self.old_gen.mark_step(usize::MAX);
        self.old_gen.sweep()
    }

    /// Trigger an old generation mark-sweep-compaction (moving, major GC).
    pub fn collect_old_gen_compact(&mut self) -> usize {
        self.old_gen.start_incremental_marking(&self.roots);
        self.old_gen.mark_step(usize::MAX);
        self.old_gen.compact()
    }

    /// Get current heap statistics.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            nursery_utilization: self.nursery.utilization(),
            old_gen_utilization: self.old_gen.utilization(),
            scavenge_count: self.nursery.scavenge_count,
            total_allocations: self.nursery.total_allocations,
            large_object_count: self.large_objects.object_count(),
        }
    }

    /// Check if a pointer is managed by this heap (in any space).
    pub fn contains(&self, ptr: *const u8) -> bool {
        self.nursery.contains(ptr) || self.old_gen.contains(ptr) || self.large_objects.contains(ptr)
    }

    /// Address range covering every space that may hold GC objects.
    ///
    /// Conservative stack scans use this as a cheap first-level filter before
    /// consulting the exact object table.
    pub fn address_bounds(&self) -> (usize, usize) {
        let nursery_lo = self.nursery.from_start as usize;
        let mut lo = nursery_lo;
        let mut hi = nursery_lo + self.nursery.space_size;
        let old_lo = self.old_gen.start as usize;
        lo = lo.min(old_lo);
        hi = hi.max(old_lo + self.old_gen.size);
        if let Some((large_lo, large_hi)) = self.large_objects.address_bounds() {
            lo = lo.min(large_lo);
            hi = hi.max(large_hi);
        }
        (lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> HeapConfig {
        HeapConfig { nursery_size: 4096, old_gen_size: 4096 }
    }

    #[test]
    fn test_heap_creation() {
        let heap = Heap::new(test_config());
        let stats = heap.stats();
        assert_eq!(stats.nursery_utilization, 0.0);
        assert_eq!(stats.total_allocations, 0);
    }

    #[test]
    fn test_heap_allocation() {
        let mut heap = Heap::new(test_config());

        let ptr = heap.allocate(32, TypeTag::Object);
        assert!(ptr.is_some());

        let ptr = ptr.unwrap();
        assert!(heap.contains(ptr));

        let stats = heap.stats();
        assert_eq!(stats.total_allocations, 1);
    }

    #[test]
    fn test_large_object_routing() {
        let mut heap = Heap::new(test_config());

        // This is larger than LARGE_OBJECT_THRESHOLD.
        let ptr = heap.allocate(16 * 1024, TypeTag::ArrayBuffer);
        assert!(ptr.is_some());

        let stats = heap.stats();
        assert_eq!(stats.large_object_count, 1);
    }

    #[test]
    fn test_auto_scavenge() {
        let mut heap = Heap::new(test_config());

        // Fill the nursery.
        for _ in 0..256 {
            let _ = heap.allocate(8, TypeTag::Raw);
        }

        // Next allocation should trigger scavenge (no roots, all collected)
        // and then succeed.
        let ptr = heap.allocate(8, TypeTag::Raw);
        assert!(ptr.is_some());
    }

    #[test]
    fn test_write_barrier_and_remembered_set() {
        let mut heap = Heap::new(test_config());

        // Allocate a child in the nursery
        let child_ptr = heap.allocate(16, TypeTag::Raw).unwrap();

        // Simulate an old generation object slot pointing to child_ptr
        let mut old_slot = child_ptr;
        unsafe {
            heap.write_barrier(&mut old_slot as *mut *mut u8);
        }

        assert_eq!(heap.remembered_set.len(), 1);

        // Run scavenge without adding child_ptr to heap.roots
        // The write barrier remembered set must preserve child_ptr!
        heap.collect_nursery();

        // Nursery object survived
        assert!(heap.nursery.contains(child_ptr) || heap.nursery.bytes_allocated > 0);
    }

    #[test]
    fn test_collect_old_gen_sweep() {
        let mut heap = Heap::new(test_config());

        let p1 = heap.allocate(16, TypeTag::Object).unwrap();
        let p2 = heap.allocate(16, TypeTag::Object).unwrap();

        let old_p1 = unsafe { heap.old_gen.promote(p1).unwrap() };
        let _old_p2 = unsafe { heap.old_gen.promote(p2).unwrap() };

        heap.add_root(old_p1);
        let reclaimed = heap.collect_old_gen_sweep();
        assert!(reclaimed > 0);
        assert_eq!(heap.old_gen.object_count, 1);
    }
}
