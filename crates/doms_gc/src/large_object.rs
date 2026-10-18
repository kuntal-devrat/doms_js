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
    pub fn sweep(&mut self) -> usize {
        let mut freed = 0usize;

        self.objects.retain(|obj| {
            if obj.marked {
                true // Keep alive.
            } else {
                // SAFETY: ptr was allocated with obj.layout.
                unsafe {
                    dealloc(obj.ptr, obj.layout);
                }
                freed += obj.layout.size();
                false // Remove from list.
            }
        });

        // Reset marks for next cycle.
        for obj in &mut self.objects {
            obj.marked = false;
        }

        self.bytes_allocated -= freed;
        freed
    }

    /// Number of live large objects.
    pub fn object_count(&self) -> usize {
        self.objects.len()
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
}
