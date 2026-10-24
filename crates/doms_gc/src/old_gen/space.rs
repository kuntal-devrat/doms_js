//! Old-gen space management: construction, promotion and space queries.

use super::*;

impl OldGen {
    /// Create a new old generation with the given size.
    pub fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, GC_ALIGN).expect("Invalid layout for old gen");

        // SAFETY: Layout is valid (checked above).
        let start = unsafe { alloc(layout) };
        if start.is_null() {
            panic!("Failed to allocate old generation ({} bytes)", size);
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
            free_chunks: Vec::new(),
            type_registry: TypeRegistry::new(),
        }
    }

    /// Create an old generation with the default size (64 MB).
    pub fn with_default_size() -> Self {
        Self::new(DEFAULT_OLD_GEN_SIZE)
    }

    /// Allocate memory for an object directly in the old generation.
    pub fn allocate(&mut self, payload_size: usize, type_tag: TypeTag) -> Option<*mut u8> {
        let total_size = (HEADER_SIZE + payload_size + GC_ALIGN - 1) & !(GC_ALIGN - 1);

        // Try to allocate from the free list first.
        for i in 0..self.free_chunks.len() {
            if self.free_chunks[i].size >= total_size {
                let chunk = self.free_chunks.remove(i);
                let remaining_sz = chunk.size - total_size;
                // Keep the block-size invariant intact: every block in the old
                // generation must have a valid header, otherwise a later sweep
                // would stop scanning at the zeroed gap and stop reclaiming
                // memory. A split remainder therefore gets its own free header;
                // a too-small tail is absorbed into this allocation.
                let alloc_size = if remaining_sz >= HEADER_SIZE + GC_ALIGN {
                    let rem = unsafe { self.start.add(chunk.offset + total_size) };
                    unsafe {
                        let rem_header = rem as *mut GcHeader;
                        ptr::write(rem_header, GcHeader::new(remaining_sz as u32, TypeTag::Raw));
                        (*rem_header).set_free();
                    }
                    self.free_chunks
                        .push(FreeChunk { offset: chunk.offset + total_size, size: remaining_sz });
                    total_size
                } else if remaining_sz > 0 {
                    chunk.size
                } else {
                    total_size
                };
                let dest = unsafe { self.start.add(chunk.offset) };
                unsafe {
                    let header = dest as *mut GcHeader;
                    ptr::write(header, GcHeader::new(alloc_size as u32, type_tag));
                    (*header).set_tenured();
                }
                self.object_count += 1;
                self.bytes_used += alloc_size;
                let payload = unsafe { dest.add(HEADER_SIZE) };
                unsafe {
                    ptr::write_bytes(payload, 0, alloc_size - HEADER_SIZE);
                }
                return Some(payload);
            }
        }

        // Check bump pointer space.
        let remaining = unsafe { self.limit.offset_from(self.cursor) } as usize;
        if total_size > remaining {
            return None;
        }

        let dest = self.cursor;
        unsafe {
            let header = dest as *mut GcHeader;
            ptr::write(header, GcHeader::new(total_size as u32, type_tag));
            (*header).set_tenured();
        }

        let payload = unsafe { dest.add(HEADER_SIZE) };
        unsafe {
            ptr::write_bytes(payload, 0, payload_size);
        }
        self.cursor = unsafe { self.cursor.add(total_size) };
        self.object_count += 1;
        self.bytes_used += total_size;

        Some(payload)
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
        let src = unsafe { payload_ptr.sub(HEADER_SIZE) };

        // Try to allocate from the free list first.
        for i in 0..self.free_chunks.len() {
            if self.free_chunks[i].size >= total_size {
                let chunk = self.free_chunks.remove(i);
                let dest = unsafe { self.start.add(chunk.offset) };
                // SAFETY: dest is within bounds, non-overlapping with nursery.
                unsafe {
                    ptr::copy_nonoverlapping(src, dest, total_size);
                    let new_header = dest as *mut GcHeader;
                    (*new_header).set_tenured();
                    (*new_header).clear_marked();
                }
                let remaining_sz = chunk.size - total_size;
                let alloc_size = if remaining_sz >= HEADER_SIZE + GC_ALIGN {
                    // Preserve the block-size invariant (see `allocate`).
                    let rem = unsafe { self.start.add(chunk.offset + total_size) };
                    unsafe {
                        let rem_header = rem as *mut GcHeader;
                        ptr::write(rem_header, GcHeader::new(remaining_sz as u32, TypeTag::Raw));
                        (*rem_header).set_free();
                    }
                    self.free_chunks
                        .push(FreeChunk { offset: chunk.offset + total_size, size: remaining_sz });
                    total_size
                } else if remaining_sz > 0 {
                    // Absorb the tail by enlarging the copied header and
                    // zeroing the remainder.
                    unsafe {
                        let new_header = dest as *mut GcHeader;
                        (*new_header).size = chunk.size as u32;
                        ptr::write_bytes(dest.add(total_size), 0, remaining_sz);
                    }
                    chunk.size
                } else {
                    total_size
                };
                self.object_count += 1;
                self.bytes_used += alloc_size;
                return Some(unsafe { dest.add(HEADER_SIZE) });
            }
        }

        // Check bump pointer space.
        let remaining = unsafe { self.limit.offset_from(self.cursor) } as usize;
        if total_size > remaining {
            return None;
        }

        // Copy header + payload via bump pointer.
        // SAFETY: Non-overlapping copy, both regions are valid.
        unsafe {
            ptr::copy_nonoverlapping(src, self.cursor, total_size);
        }

        // Mark the new copy as tenured.
        let new_header = self.cursor as *mut GcHeader;
        // SAFETY: We just copied a valid header here.
        unsafe {
            (*new_header).set_tenured();
            (*new_header).clear_marked();
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
}
