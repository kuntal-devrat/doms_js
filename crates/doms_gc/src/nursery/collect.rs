//! Nursery collection: in-place non-moving mark and sweep.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::*;

impl Nursery {
    /// Perform a collection (minor GC) in-place without moving objects.
    ///
    /// Live objects are marked in-place. Unmarked objects are swept into `free_chunks`.
    /// Pointers never change, eliminating pointer invalidation and dangling references.
    ///
    /// # Arguments
    /// - `roots`: Mutable slice of root pointers. Since objects do not move,
    ///   roots remain valid and unchanged.
    ///
    /// # Returns
    /// The number of live bytes that survived.
    pub fn scavenge(&mut self, roots: &mut [*mut u8]) -> usize {
        // Step 1: Mark phase
        let mut worklist = Vec::new();

        for &root in roots.iter() {
            if !root.is_null() && self.contains(root) {
                let header = unsafe { &mut *((root as *mut GcHeader).sub(1)) };
                if !header.is_marked() {
                    header.set_marked();
                    worklist.push(root);
                }
            }
        }

        while let Some(ptr) = worklist.pop() {
            let header = unsafe { &*((ptr as *const GcHeader).sub(1)) };
            let tag = header.tag();
            unsafe {
                self.type_registry.trace(tag, ptr, &mut |slot: *mut *mut u8| {
                    if !slot.is_null() {
                        let child = *slot;
                        if !child.is_null() && self.contains(child) {
                            let child_header = &mut *((child as *mut GcHeader).sub(1));
                            if !child_header.is_marked() {
                                child_header.set_marked();
                                worklist.push(child);
                            }
                        }
                    }
                });
            }
        }

        // Step 2: In-place sweep phase using sweep_marked
        self.sweep_marked()
    }

    /// In-place sweep of the nursery based on GcHeader mark bits.
    ///
    /// If no live objects survived in the nursery, cursor is reset to from_start (fast reset).
    /// Otherwise, dead objects are marked free and added to `free_chunks`.
    pub fn sweep_marked(&mut self) -> usize {
        let mut scan = self.from_start;
        let mut live_bytes = 0;
        let mut live_count = 0;
        let mut current_free_offset: Option<usize> = None;
        let mut current_free_size = 0;
        self.free_chunks.clear();

        while (scan as usize) < (self.cursor as usize) {
            let header = unsafe { &mut *(scan as *mut GcHeader) };
            let sz = header.total_size();
            if sz == 0 {
                break;
            }
            let offset = unsafe { scan.offset_from(self.from_start) as usize };
            if header.is_marked() {
                header.clear_marked();
                live_bytes += sz;
                live_count += 1;
                if let Some(off) = current_free_offset.take() {
                    self.free_chunks.push(FreeChunk {
                        offset: off,
                        size: current_free_size,
                    });
                    current_free_size = 0;
                }
            } else {
                header.set_free();
                let payload = unsafe { scan.add(HEADER_SIZE) };
                unsafe { ptr::write_bytes(payload, 0, sz - HEADER_SIZE); }
                if current_free_offset.is_none() {
                    current_free_offset = Some(offset);
                }
                current_free_size += sz;
            }
            scan = unsafe { scan.add(sz) };
        }

        if let Some(off) = current_free_offset {
            self.free_chunks.push(FreeChunk {
                offset: off,
                size: current_free_size,
            });
        }

        if live_count == 0 {
            // Entire nursery is dead: fast reset!
            self.cursor = self.from_start;
            self.free_chunks.clear();
            self.bytes_allocated = 0;
        } else {
            self.bytes_allocated = live_bytes;
        }

        self.scavenge_count += 1;
        live_bytes
    }

    /// Complete a collection pass (compatibility helper).
    pub fn finish_scavenge(&mut self, _to_cursor: *mut u8) -> usize {
        self.scavenge_count += 1;
        self.bytes_allocated
    }

    /// Non-moving object "copy" (returns existing pointer directly).
    #[inline]
    pub fn copy_object(
        &self,
        payload_ptr: *mut u8,
        _to_cursor: &mut *mut u8,
        _to_limit: *const u8,
    ) -> *mut u8 {
        payload_ptr
    }
}
