//! Sweeping and compaction for the old generation.

use super::*;

impl OldGen {
    /// Compact the old generation by sliding marked objects toward the beginning.
    /// Unmarked objects are collected.
    /// Returns the number of bytes reclaimed.
    pub fn compact(&mut self) -> usize {
        let mut scan = self.start;
        let mut free = self.start;
        let mut live_bytes = 0;
        let mut live_count = 0;

        while scan < self.cursor {
            unsafe {
                let header = &mut *(scan as *mut GcHeader);
                let sz = header.total_size();
                if sz == 0 {
                    break;
                }
                if header.is_marked() {
                    header.clear_marked();
                    if scan != free {
                        ptr::copy(scan, free, sz);
                    }
                    free = free.add(sz);
                    live_bytes += sz;
                    live_count += 1;
                }
                scan = scan.add(sz);
            }
        }

        let reclaimed = self.bytes_used.saturating_sub(live_bytes);
        self.cursor = free;
        self.bytes_used = live_bytes;
        self.object_count = live_count;
        self.mark_state = MarkState::Idle;
        self.mark_worklist.clear();
        self.free_chunks.clear();
        reclaimed
    }

    /// Sweep unmarked objects without moving live objects.
    ///
    /// Reclaims memory from unmarked objects into the free list, resetting
    /// mark bits for live objects. Unlike `compact()`, `sweep()` does not slide
    /// objects, ensuring that existing pointers remain completely valid.
    ///
    /// Freed blocks keep their header (with the free flag set) so that the
    /// sweep can be repeated: every block in the space always carries a valid
    /// size, which lets the next sweep keep scanning past it.
    /// Returns the number of bytes reclaimed.
    pub fn sweep(&mut self) -> usize {
        let mut scan = self.start;
        let mut live_bytes = 0;
        let mut live_count = 0;
        let mut reclaimed_bytes = 0;
        self.free_chunks.clear();

        while scan < self.cursor {
            unsafe {
                let header = &mut *(scan as *mut GcHeader);
                let sz = header.total_size();
                if sz == 0 {
                    break;
                }
                let offset = scan.offset_from(self.start) as usize;
                if header.is_free() {
                    // Freed by an earlier sweep: carry the block over.
                    header.clear_marked();
                    self.free_chunks.push(FreeChunk { offset, size: sz });
                } else if header.is_marked() {
                    header.clear_marked();
                    live_bytes += sz;
                    live_count += 1;
                } else {
                    // Mark bit not set: object is dead. Keep the header (so
                    // this scan can be repeated) but zero the payload.
                    header.set_free();
                    ptr::write_bytes(scan.add(HEADER_SIZE), 0, sz - HEADER_SIZE);
                    self.free_chunks.push(FreeChunk { offset, size: sz });
                    reclaimed_bytes += sz;
                }
                scan = scan.add(sz);
            }
        }

        self.bytes_used = live_bytes;
        self.object_count = live_count;
        self.mark_state = MarkState::Idle;
        self.mark_worklist.clear();
        reclaimed_bytes
    }
}
