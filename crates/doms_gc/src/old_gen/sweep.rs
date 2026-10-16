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
    /// Returns the number of bytes reclaimed.
    pub fn sweep(&mut self) -> usize {
        let mut scan = self.start;
        let mut live_bytes = 0;
        let mut live_count = 0;
        let mut reclaimed_bytes = 0;
        self.free_chunks.clear();

        let mut current_free_offset: Option<usize> = None;
        let mut current_free_size = 0;

        while scan < self.cursor {
            unsafe {
                let header = &mut *(scan as *mut GcHeader);
                let sz = header.total_size();
                if sz == 0 {
                    break;
                }
                let offset = scan.offset_from(self.start) as usize;
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
                    // Mark bit not set: object is dead. Zero its header/payload and add to free list.
                    ptr::write_bytes(scan, 0, sz);
                    reclaimed_bytes += sz;
                    if current_free_offset.is_none() {
                        current_free_offset = Some(offset);
                    }
                    current_free_size += sz;
                }
                scan = scan.add(sz);
            }
        }

        if let Some(off) = current_free_offset {
            self.free_chunks.push(FreeChunk {
                offset: off,
                size: current_free_size,
            });
        }

        self.bytes_used = live_bytes;
        self.object_count = live_count;
        self.mark_state = MarkState::Idle;
        self.mark_worklist.clear();
        reclaimed_bytes
    }
}
