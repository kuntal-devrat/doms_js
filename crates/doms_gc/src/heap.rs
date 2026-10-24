//! Top-level Heap — manages all GC memory spaces.
//!
//! The [`Heap`] is the primary interface for allocating GC-managed objects.
//! It routes allocations to the appropriate space (nursery, old gen, or
//! large object space) and triggers collections when needed.

use crate::header::{GcHeader, TypeTag};
use crate::large_object::{LARGE_OBJECT_THRESHOLD, LargeObjectSpace};
use crate::nursery::Nursery;
use crate::old_gen::OldGen;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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

/// Coordinator for background concurrent GC marking in the old generation.
pub struct ConcurrentMarker {
    worker: Option<std::thread::JoinHandle<()>>,
    stop_signal: Arc<AtomicBool>,
    done_flag: Arc<AtomicBool>,
    shared_worklist: Arc<Mutex<Vec<usize>>>,
}

impl ConcurrentMarker {
    pub fn push_work(&self, ptr: *mut u8) {
        if let Ok(mut worklist) = self.shared_worklist.lock() {
            worklist.push(ptr as usize);
            self.done_flag.store(false, Ordering::Relaxed);
        }
    }

    pub fn is_done(&self) -> bool {
        self.done_flag.load(Ordering::SeqCst)
            && self.shared_worklist.lock().map(|w| w.is_empty()).unwrap_or(true)
    }

    pub fn stop_and_join(&mut self) -> Vec<*mut u8> {
        self.stop_signal.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.shared_worklist
            .lock()
            .map(|mut w| w.drain(..).map(|addr| addr as *mut u8).collect())
            .unwrap_or_default()
    }
}

impl Drop for ConcurrentMarker {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
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
    /// Background concurrent marking coordinator.
    pub concurrent_marker: Option<ConcurrentMarker>,
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
            concurrent_marker: None,
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

    /// Register a type tracer for interior reference tracing in the nursery and old generation.
    pub fn register_tracer(&mut self, tag: TypeTag, visitor: crate::trace::TraceVisitor) {
        self.nursery.register_tracer(tag, visitor);
        self.old_gen.register_tracer(tag, visitor);
    }

    /// Post-write barrier: records when an object is updated.
    ///
    /// 1. Generational: If target is in nursery, remembers the slot.
    /// 2. Concurrent/Incremental: If target is in old generation and marking is active,
    ///    marks target and adds it to the marking worklist (Dijkstra SATB write barrier).
    ///
    /// # Safety
    /// `slot` must be a valid, readable pointer to a pointer.
    pub unsafe fn write_barrier(&mut self, slot: *mut *mut u8) {
        if slot.is_null() {
            return;
        }
        let target = unsafe { *slot };
        if target.is_null() {
            return;
        }
        if self.nursery.contains(target) {
            self.remembered_set.remember(slot);
        }
        if let Some(marker) = &self.concurrent_marker {
            if self.old_gen.contains(target) {
                let header = unsafe { &*((target as *const GcHeader).sub(1)) };
                if header.set_marked_atomic() {
                    marker.push_work(target);
                }
            }
        } else if self.old_gen.mark_state == crate::old_gen::MarkState::MarkingObjects {
            self.old_gen.mark_object(target);
        }
    }

    /// Begin incremental marking for the old generation using the registered roots.
    pub fn start_incremental_marking(&mut self) {
        self.old_gen.start_incremental_marking(&self.roots);
    }

    /// Perform a single step of incremental marking with a slice budget.
    /// Returns `true` if marking is complete, `false` if more work remains.
    pub fn step_incremental_marking(&mut self, slice_limit: usize) -> bool {
        self.old_gen.mark_step(slice_limit)
    }

    /// Check if incremental marking is complete.
    pub fn is_incremental_marking_complete(&self) -> bool {
        self.old_gen.mark_state == crate::old_gen::MarkState::Complete
    }

    /// Start concurrent background marking on a dedicated worker thread.
    pub fn start_concurrent_marking(&mut self) {
        if self.concurrent_marker.is_some() {
            return;
        }

        self.old_gen.mark_state = crate::old_gen::MarkState::MarkingObjects;
        self.old_gen.mark_worklist.clear();

        let mut initial_worklist: Vec<usize> = Vec::new();
        for &root in &self.roots {
            if !root.is_null() && self.old_gen.contains(root) {
                unsafe {
                    let header = &mut *((root as *mut GcHeader).sub(1));
                    if header.set_marked_atomic() {
                        initial_worklist.push(root as usize);
                    }
                }
            }
        }

        let stop_signal = Arc::new(AtomicBool::new(false));
        let done_flag = Arc::new(AtomicBool::new(false));
        let shared_worklist = Arc::new(Mutex::new(initial_worklist));

        let stop_clone = stop_signal.clone();
        let done_clone = done_flag.clone();
        let work_clone = shared_worklist.clone();

        let start = self.old_gen.start as usize;
        let limit = self.old_gen.limit as usize;
        let type_registry = self.old_gen.type_registry;

        let handle = std::thread::Builder::new()
            .name("doms-concurrent-gc-marker".into())
            .spawn(move || {
                let mut local_batch = Vec::with_capacity(64);
                let mut new_children = Vec::new();

                while !stop_clone.load(Ordering::Relaxed) {
                    local_batch.clear();
                    if let Ok(mut worklist) = work_clone.lock() {
                        let to_drain = worklist.len().min(64);
                        if to_drain == 0 {
                            done_clone.store(true, Ordering::SeqCst);
                        } else {
                            done_clone.store(false, Ordering::Relaxed);
                            let start_idx = worklist.len() - to_drain;
                            local_batch.extend(worklist.drain(start_idx..));
                        }
                    }

                    if local_batch.is_empty() {
                        std::thread::yield_now();
                        std::thread::sleep(std::time::Duration::from_micros(100));
                        continue;
                    }

                    new_children.clear();
                    for addr in local_batch.drain(..) {
                        if addr < start || addr >= limit {
                            continue;
                        }
                        let ptr = addr as *mut u8;
                        let header = unsafe { &*((ptr as *const GcHeader).sub(1)) };
                        let tag = header.tag();
                        unsafe {
                            type_registry.trace(tag, ptr, &mut |slot: *mut *mut u8| {
                                if !slot.is_null() {
                                    let child = *slot;
                                    let child_addr = child as usize;
                                    if !child.is_null()
                                        && child_addr >= start
                                        && child_addr < limit
                                    {
                                        let child_header =
                                            &*((child as *const GcHeader).sub(1));
                                        if child_header.set_marked_atomic() {
                                            new_children.push(child_addr);
                                        }
                                    }
                                }
                            });
                        }
                    }

                    if !new_children.is_empty() {
                        if let Ok(mut worklist) = work_clone.lock() {
                            worklist.extend(new_children.drain(..));
                        }
                    }
                }
            })
            .ok();

        self.concurrent_marker = Some(ConcurrentMarker {
            worker: handle,
            stop_signal,
            done_flag,
            shared_worklist,
        });
    }

    /// Check if background concurrent marking is currently active.
    pub fn is_concurrent_marking_active(&self) -> bool {
        self.concurrent_marker.is_some()
    }

    /// Check if background concurrent marking has drained all known reachable objects.
    pub fn is_concurrent_marking_done(&self) -> bool {
        self.concurrent_marker
            .as_ref()
            .map(|m| m.is_done())
            .unwrap_or(false)
    }

    /// Step concurrent marking from the main thread, assisting the worker thread by
    /// processing up to `slice_limit` items. Returns `true` if marking is complete.
    pub fn step_concurrent_marking(&mut self, slice_limit: usize) -> bool {
        if self.is_concurrent_marking_done() {
            self.finish_concurrent_marking();
            return true;
        }
        if let Some(marker) = &self.concurrent_marker {
            let mut items = Vec::new();
            if let Ok(mut w) = marker.shared_worklist.lock() {
                let to_take = w.len().min(slice_limit);
                let start_idx = w.len() - to_take;
                items.extend(w.drain(start_idx..).map(|addr| addr as *mut u8));
            }
            if !items.is_empty() {
                self.old_gen.mark_worklist.extend(items);
                self.old_gen.mark_step(slice_limit);
            }
        }
        self.is_concurrent_marking_done()
    }

    /// Finish concurrent marking: joins background thread, performs final remark,
    /// and completes the marking phase.
    pub fn finish_concurrent_marking(&mut self) -> bool {
        let remaining = if let Some(mut marker) = self.concurrent_marker.take() {
            marker.stop_and_join()
        } else {
            return false;
        };

        // Final remark STW pass
        self.old_gen.mark_worklist.extend(remaining);
        self.old_gen.mark_step(usize::MAX);
        self.old_gen.mark_state = crate::old_gen::MarkState::Complete;
        true
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
