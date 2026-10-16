//! Incremental marking for the old generation.

use super::*;

impl OldGen {
    /// Begin an incremental marking phase from the given root pointers.
    pub fn start_incremental_marking(&mut self, roots: &[*mut u8]) {
        self.mark_state = MarkState::MarkingObjects;
        self.mark_worklist.clear();
        for &root in roots {
            if !root.is_null() && self.contains(root) {
                unsafe {
                    let header = &mut *((root as *mut GcHeader).sub(1));
                    if !header.is_marked() {
                        header.set_marked();
                        self.mark_worklist.push(root);
                    }
                }
            }
        }
        if self.mark_worklist.is_empty() {
            self.mark_state = MarkState::Complete;
        }
    }

    /// Perform a single step of incremental marking, processing at most `work_limit` objects.
    /// Returns `true` if marking is complete, `false` if more work remains.
    pub fn mark_step(&mut self, work_limit: usize) -> bool {
        if self.mark_state != MarkState::MarkingObjects {
            return self.mark_state == MarkState::Complete;
        }

        let mut work_done = 0;
        while let Some(_ptr) = self.mark_worklist.pop() {
            work_done += 1;
            if self.mark_worklist.is_empty() {
                self.mark_state = MarkState::Complete;
                return true;
            }
            if work_done >= work_limit {
                return false;
            }
        }

        self.mark_state = MarkState::Complete;
        true
    }

    /// Reset all mark bits and the marking state back to idle.
    pub fn reset_marks(&mut self) {
        let mut curr = self.start;
        while curr < self.cursor {
            unsafe {
                let header = &mut *(curr as *mut GcHeader);
                header.clear_marked();
                let sz = header.total_size();
                if sz == 0 {
                    break;
                }
                curr = curr.add(sz);
            }
        }
        self.mark_state = MarkState::Idle;
        self.mark_worklist.clear();
    }

    /// Check if an object in the old generation is marked.
    pub fn is_marked(&self, payload_ptr: *const u8) -> bool {
        if payload_ptr.is_null() || !self.contains(payload_ptr) {
            return false;
        }
        unsafe {
            let header = &*((payload_ptr as *const GcHeader).sub(1));
            header.is_marked()
        }
    }
}
