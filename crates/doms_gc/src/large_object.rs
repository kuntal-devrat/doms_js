//! Large Object Space — for objects too big for the nursery.
//!
//! Objects larger than [`LARGE_OBJECT_THRESHOLD`] are allocated directly
//! in the large object space. They are never moved (pinned), which avoids
//! the cost of copying large buffers during scavenge.


use crate::header::{GC_ALIGN, GcHeader, HEADER_SIZE, TypeTag};
use std::alloc::{Layout, alloc, dealloc};
use std::ptr;

/// Objects larger than 8 KB go to the large object space.
pub const LARGE_OBJECT_THRESHOLD: usize = 8 * 1024;

/// A single large object allocation.
struct LargeObject {
    /// Pointer to the allocation (header + payload).
    ptr: *mut u8,
    /// Layout used for this allocation (needed for dealloc).
    layout: Layout,
    /// Whether this object is marked (alive) during GC.
    marked: bool,
}

/// The large object space.
pub struct LargeObjectSpace {
    /// All live large object allocations.
    objects: Vec<LargeObject>,
    /// Total bytes allocated.
    pub bytes_allocated: usize,
}

impl LargeObjectSpace {
    /// Create a new, empty large object space.
    pub fn new() -> Self {
        Self { objects: Vec::new(), bytes_allocated: 0 }
    }

    /// Allocate a large object.
    ///
    /// Returns a pointer to the payload (after the header), or `None` on OOM.
    pub fn allocate(&mut self, payload_size: usize, type_tag: TypeTag) -> Option<*mut u8> {
        let total_size = HEADER_SIZE + payload_size;
        let layout = Layout::from_size_align(total_size, GC_ALIGN).ok()?;

        // SAFETY: Layout is valid (checked above).
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return None;
        }

        // Write header.
        // SAFETY: ptr is valid for total_size bytes.
        unsafe {
            let header = ptr as *mut GcHeader;
            ptr::write(header, GcHeader::new(total_size as u32, type_tag));
            (*header).set_pinned(); // Large objects are never moved.
        }

        let payload = unsafe { ptr.add(HEADER_SIZE) };

        // Zero-initialize payload.
        // SAFETY: payload is valid for payload_size bytes.
        unsafe {
            ptr::write_bytes(payload, 0, payload_size);
        }

        self.objects.push(LargeObject { ptr, layout, marked: false });
        self.bytes_allocated += total_size;

        Some(payload)
    }

    /// Check if a pointer falls within a large object.
    pub fn contains(&self, ptr: *const u8) -> bool {
        self.objects.iter().any(|obj| {
            let start = obj.ptr as usize;
            let end = start + obj.layout.size();
            let addr = ptr as usize;
            addr >= start && addr < end
        })
    }

    /// Mark a large object as alive.
    pub fn mark(&mut self, payload_ptr: *const u8) -> bool {
        let header_addr = (payload_ptr as usize).wrapping_sub(HEADER_SIZE);
        for obj in &mut self.objects {
            if obj.ptr as usize == header_addr {
                obj.marked = true;
                return true;
            }
        }
        false
    }

    /// Sweep: free all unmarked large objects, reset marks on survivors.
    ///
    /// A large object is considered live when either its per-space mark flag
    /// or its [`GcHeader`] mark bit is set. The header bit lets the VM's
    /// tracing collector mark large objects without reaching into this crate.
    pub fn sweep(&mut self) -> usize {
        let mut freed = 0usize;

        let mut i = 0;
        while i < self.objects.len() {
            let header_marked = unsafe {
                let header = &mut *(self.objects[i].ptr as *mut GcHeader);
                let marked = header.is_marked();
                header.clear_marked();
                marked
            };
            if self.objects[i].marked || header_marked {
                self.objects[i].marked = false;
                i += 1;
            } else {
                let obj = self.objects.swap_remove(i);
                // SAFETY: ptr was allocated with obj.layout.
                unsafe {
                    dealloc(obj.ptr, obj.layout);
                }
                freed += obj.layout.size();
            }
        }

        self.bytes_allocated -= freed;
        freed
    }

    /// Number of live large objects.
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Address range covering every large object allocation, if any.
    ///
    /// Used as a cheap first-level filter by conservative stack scans.
    pub fn address_bounds(&self) -> Option<(usize, usize)> {
        let mut lo = usize::MAX;
        let mut hi = 0usize;
        for obj in &self.objects {
            lo = lo.min(obj.ptr as usize);
            hi = hi.max(obj.ptr as usize + obj.layout.size());
        }
        if lo < hi {
            Some((lo, hi))
        } else {
            None
        }
    }
}

impl Default for LargeObjectSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LargeObjectSpace {
    fn drop(&mut self) {
        for obj in &self.objects {
            // SAFETY: Each ptr was allocated with its corresponding layout.
            unsafe {
                dealloc(obj.ptr, obj.layout);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_large_object_allocation() {
        let mut los = LargeObjectSpace::new();

        let ptr = los.allocate(16 * 1024, TypeTag::ArrayBuffer);
        assert!(ptr.is_some());
        assert_eq!(los.object_count(), 1);

        let ptr = ptr.unwrap();
        assert!(los.contains(ptr));
    }

    #[test]
    fn test_large_object_sweep() {
        let mut los = LargeObjectSpace::new();

        let p1 = los.allocate(16 * 1024, TypeTag::ArrayBuffer).unwrap();
        let _p2 = los.allocate(16 * 1024, TypeTag::ArrayBuffer).unwrap();

        // Mark only p1.
        los.mark(p1);

        let freed = los.sweep();
        assert!(freed > 0);
        assert_eq!(los.object_count(), 1);
        assert!(los.contains(p1));
    }

    #[test]
    fn test_large_object_sweep_honours_header_mark_bits() {
        let mut los = LargeObjectSpace::new();

        let p1 = los.allocate(16 * 1024, TypeTag::ArrayBuffer).unwrap();
        let _p2 = los.allocate(16 * 1024, TypeTag::ArrayBuffer).unwrap();

        // Mimic an external tracing collector: set the header mark bit of p1.
        unsafe {
            (*(p1 as *mut GcHeader).sub(1)).set_marked();
        }
        let freed = los.sweep();
        assert!(freed > 0);
        assert_eq!(los.object_count(), 1);
        assert!(los.contains(p1));
        // Marks must be reset so a later sweep does not keep stale survivors.
        unsafe {
            assert!(!(*(p1 as *mut GcHeader).sub(1)).is_marked());
        }
    }
}
