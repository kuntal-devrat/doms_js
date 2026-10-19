//! Nursery allocation: construction, bump allocation and value writes.

use super::*;

impl Nursery {
    /// Create a new nursery with the given space size.
    ///
    /// Allocates a contiguous memory region of `space_size` bytes.
    ///
    /// # Panics
    /// Panics if memory allocation fails.
    pub fn new(space_size: usize) -> Self {
        assert!(space_size >= 4096, "Nursery size must be at least 4KB");
        assert!(
            space_size.is_power_of_two() || space_size % 4096 == 0,
            "Nursery size should be page-aligned"
        );

        let layout =
            Layout::from_size_align(space_size, GC_ALIGN).expect("Invalid layout for nursery");

        // SAFETY: We allocate one contiguous memory region. Layout is valid
        // (checked above). We check for null.
        let from_start = unsafe { alloc(layout) };

        if from_start.is_null() {
            panic!("Failed to allocate nursery ({} bytes)", space_size);
        }

        // Zero-initialize space for safety.
        // SAFETY: Pointer is valid for space_size bytes.
        unsafe {
            ptr::write_bytes(from_start, 0, space_size);
        }

        let limit = unsafe { from_start.add(space_size) };

        Self {
            from_start,
            space_size,
            cursor: from_start,
            limit,
            free_chunks: Vec::new(),
            bytes_allocated: 0,
            scavenge_count: 0,
            total_allocations: 0,
            type_registry: TypeRegistry::new(),
        }
    }

    /// Register a trace visitor for interior reference tracing during collection.
    pub fn register_tracer(&mut self, tag: TypeTag, visitor: TraceVisitor) {
        self.type_registry.register(tag, visitor);
    }

    /// Create a nursery with the default size (2 MB).
    pub fn with_default_size() -> Self {
        Self::new(DEFAULT_NURSERY_SIZE)
    }

    /// Allocate `payload_size` bytes in the nursery.
    ///
    /// Returns a pointer to the payload (after the GcHeader) or `None` if
    /// the nursery is full and a collection is needed.
    ///
    /// The allocation is aligned to [`GC_ALIGN`] bytes.
    ///
    /// # Arguments
    /// - `payload_size`: Size of the object payload in bytes (excluding header).
    /// - `type_tag`: Type tag for the object (used during GC tracing).
    pub fn allocate(&mut self, payload_size: usize, type_tag: TypeTag) -> Option<*mut u8> {
        let total_size = Self::align_up(HEADER_SIZE + payload_size, GC_ALIGN);

        // 1. Try to allocate from the free list first
        for i in 0..self.free_chunks.len() {
            if self.free_chunks[i].size >= total_size {
                let chunk = self.free_chunks.remove(i);
                let dest = unsafe { self.from_start.add(chunk.offset) };
                unsafe {
                    let header = dest as *mut GcHeader;
                    ptr::write(header, GcHeader::new(total_size as u32, type_tag));
                }
                let remaining_sz = chunk.size - total_size;
                if remaining_sz >= HEADER_SIZE + GC_ALIGN {
                    self.free_chunks.push(FreeChunk {
                        offset: chunk.offset + total_size,
                        size: remaining_sz,
                    });
                }
                self.bytes_allocated += total_size;
                self.total_allocations += 1;
                let payload = unsafe { dest.add(HEADER_SIZE) };
                unsafe { ptr::write_bytes(payload, 0, payload_size); }
                return Some(payload);
            }
        }

        // 2. Bump pointer allocation
        if self.cursor >= self.limit {
            return None;
        }
        let remaining = unsafe { self.limit.offset_from(self.cursor) } as usize;
        if total_size > remaining {
            return None; // Nursery full — caller should trigger collection.
        }

        let header_ptr = self.cursor as *mut GcHeader;

        // Write the object header.
        // SAFETY: cursor points to valid, writable memory within nursery.
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

    /// Round up `size` to the nearest multiple of `align`.
    #[inline]
    const fn align_up(size: usize, align: usize) -> usize {
        (size + align - 1) & !(align - 1)
    }
}
