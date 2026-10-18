//! Nursery inspection: containment checks, usage stats and iteration.

use super::*;

impl Nursery {
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

    /// Returns the start pointer of from-space.
    #[inline]
    pub fn from_start(&self) -> *mut u8 {
        self.from_start
    }

    /// Returns the start pointer of to-space.
    #[inline]
    pub fn to_start(&self) -> *mut u8 {
        self.to_start
    }

    /// Returns the limit pointer of to-space.
    #[inline]
    pub fn to_limit(&self) -> *mut u8 {
        unsafe { self.to_start.add(self.space_size) }
    }

    /// Returns the semi-space size.
    #[inline]
    pub fn space_size(&self) -> usize {
        self.space_size
    }

    /// Returns the current bump pointer cursor.
    #[inline]
    pub fn cursor(&self) -> *mut u8 {
        self.cursor
    }

    /// Sets the bump pointer cursor.
    #[inline]
    pub fn set_cursor(&mut self, cursor: *mut u8) {
        if cursor > self.cursor {
            self.cursor = cursor;
        }
    }

    /// Returns the end limit of from-space.
    #[inline]
    pub fn limit(&self) -> *mut u8 {
        self.limit
    }

    /// Returns the utilization ratio (0.0 to 1.0).
    #[inline]
    pub fn utilization(&self) -> f64 {
        self.used_bytes() as f64 / self.space_size as f64
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
}
