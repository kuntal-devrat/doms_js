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

    /// Register a trace visitor function for interior reference tracing.
    pub fn register_tracer(&mut self, tag: TypeTag, visitor: TraceVisitor) {
        self.type_registry.register(tag, visitor);
    }

    /// Mark an object in the old generation and enqueue it into the mark worklist
    /// if incremental/concurrent marking is currently active.
    pub fn mark_object(&mut self, payload_ptr: *mut u8) {
        if payload_ptr.is_null() || !self.contains(payload_ptr) {
            return;
        }
        unsafe {
            let header = &mut *((payload_ptr as *mut GcHeader).sub(1));
            if !header.is_marked() {
                header.set_marked();
                if self.mark_state == MarkState::MarkingObjects {
                    self.mark_worklist.push(payload_ptr);
                }
            }
        }
    }

    /// Perform a single step of incremental marking, processing at most `work_limit` objects.
    /// Traces outgoing object references via the registered `type_registry`.
    /// Returns `true` if marking is complete, `false` if more work remains.
    pub fn mark_step(&mut self, work_limit: usize) -> bool {
        if self.mark_state != MarkState::MarkingObjects {
            return self.mark_state == MarkState::Complete;
        }

        let start = self.start as usize;
        let limit = self.limit as usize;
        let type_registry = self.type_registry;
        let mut work_done = 0;

        while let Some(ptr) = self.mark_worklist.pop() {
            work_done += 1;

            let header = unsafe { &*((ptr as *const GcHeader).sub(1)) };
            let tag = header.tag();
            let worklist = &mut self.mark_worklist;
            unsafe {
                type_registry.trace(tag, ptr, &mut |slot: *mut *mut u8| {
                    if !slot.is_null() {
                        let child = *slot;
                        let c_addr = child as usize;
                        if !child.is_null() && c_addr >= start && c_addr < limit {
                            let child_header = &mut *((child as *mut GcHeader).sub(1));
                            if !child_header.is_marked() {
                                child_header.set_marked();
                                worklist.push(child);
                            }
                        }
                    }
                });
            }

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

    /// Perform a step of incremental marking with a custom tracing callback.
    /// `tracer` is called with `(object_ptr, &mut child_collector)` for each popped object.
    pub fn mark_step_with_tracer<F>(&mut self, work_limit: usize, mut tracer: F) -> bool
    where
        F: FnMut(*mut u8, &mut Vec<*mut u8>),
    {
        if self.mark_state != MarkState::MarkingObjects {
            return self.mark_state == MarkState::Complete;
        }

        let mut children = Vec::new();
        let mut work_done = 0;
        while let Some(ptr) = self.mark_worklist.pop() {
            work_done += 1;
            children.clear();
            tracer(ptr, &mut children);

            for &child in &children {
                if !child.is_null() && self.contains(child) {
                    unsafe {
                        let child_header = &mut *((child as *mut GcHeader).sub(1));
                        if !child_header.is_marked() {
                            child_header.set_marked();
                            self.mark_worklist.push(child);
                        }
                    }
                }
            }

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
