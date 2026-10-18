//! Nursery scavenging: Cheney copying collection and object forwarding.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::*;

impl Nursery {
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

        // Step 1: Copy all root-referenced objects to to-space.
        for root in roots.iter_mut() {
            if root.is_null() {
                continue;
            }
            if self.contains(*root) {
                *root = self.copy_object(*root, &mut to_cursor, to_limit);
            }
        }

        // Step 2: Cheney scan — walk the copied objects in to-space and
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

        // Step 3: Swap from-space and to-space.
        std::mem::swap(&mut self.from_start, &mut self.to_start);
        self.cursor = to_cursor;
        self.limit = unsafe { self.from_start.add(self.space_size) };

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

    /// Complete a custom scavenge pass, swapping from/to spaces and updating cursors.
    pub fn finish_scavenge(&mut self, to_cursor: *mut u8) -> usize {
        std::mem::swap(&mut self.from_start, &mut self.to_start);
        self.cursor = to_cursor;
        self.limit = unsafe { self.from_start.add(self.space_size) };
        self.scavenge_count += 1;
        let survived = unsafe { to_cursor.offset_from(self.from_start) } as usize;
        self.bytes_allocated = survived;
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
    pub fn copy_object(
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

        let cur_age = header.age();

        // Copy the entire object (header + payload) to to-space.
        let new_header_ptr = *to_cursor;
        // SAFETY: Both pointers are valid, non-overlapping, and to-space has space.
        unsafe {
            ptr::copy_nonoverlapping(header_ptr as *const u8, new_header_ptr, total_size);
            let new_header = &mut *(new_header_ptr as *mut GcHeader);
            new_header.set_age((cur_age + 1).min(15));
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
}
