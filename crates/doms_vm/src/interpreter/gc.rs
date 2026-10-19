//! Object tracking and tracing garbage collection entry points.

use super::*;

impl Vm {
    // Allocation helpers

    #[inline]
    pub fn track_ptr(&mut self, ptr: *mut JsObject) {
        self.objects.push(ptr);
        if let Some(max) = self.max_objects {
            if self.objects.len() >= max {
                self.oom = true;
            }
        }
    }

    /// Perform a full tracing garbage collection cycle across all live roots.
    pub fn gc_collect(&mut self) -> usize {
        use std::collections::HashSet;

        let mut live = HashSet::new();
        let mut worklist = Vec::new();

        let mark_val = |val: JsValue, worklist: &mut Vec<*mut JsObject>, live: &mut HashSet<*mut JsObject>| {
            if let Some(ptr) = val.as_heap_ptr() {
                let obj_ptr = ptr as *mut JsObject;
                if live.insert(obj_ptr) {
                    worklist.push(obj_ptr);
                    unsafe {
                        let header = &mut *((obj_ptr as *mut doms_gc::GcHeader).sub(1));
                        header.set_marked();
                    }
                }
            }
        };

        // 1. Mark Roots
        if let Some(pending) = self.pending_alloc {
            if live.insert(pending) {
                worklist.push(pending);
                unsafe {
                    let header = &mut *((pending as *mut doms_gc::GcHeader).sub(1));
                    header.set_marked();
                }
            }
        }
        if let Some(proto) = self.object_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.array_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.function_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.regexp_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.string_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.number_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.boolean_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.symbol_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.promise_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.date_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.iterator_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.array_iterator_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.map_iterator_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.set_iterator_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.string_iterator_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.generator_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.wrap_for_valid_iterator_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(proto) = self.iterator_helper_proto {
            mark_val(proto, &mut worklist, &mut live);
        }
        if let Some(nt) = self.current_new_target {
            mark_val(nt, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_iterator {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_async_iterator {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_to_string_tag {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_is_concat_spreadable {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_species {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_to_primitive {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_has_instance {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_match {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_match_all {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_replace {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_search {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_split {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_unscopables {
            mark_val(s, &mut worklist, &mut live);
        }
        for s in self.symbol_registry.values() {
            mark_val(*s, &mut worklist, &mut live);
        }
        for val in self.global_lexicals.values() {
            mark_val(*val, &mut worklist, &mut live);
        }
        for val in &self.temp_roots {
            mark_val(*val, &mut worklist, &mut live);
        }
        for val in &self.stack {
            mark_val(*val, &mut worklist, &mut live);
        }
        mark_val(self.global_object, &mut worklist, &mut live);
        for frame in &self.frames {
            if let Some(gv) = frame.gen_val {
                mark_val(gv, &mut worklist, &mut live);
            }
            if let Some(cv) = frame.callee_val {
                mark_val(cv, &mut worklist, &mut live);
            }
            mark_val(frame.this_val, &mut worklist, &mut live);
            for local in &frame.locals {
                mark_val(*local, &mut worklist, &mut live);
            }
            for scope in &frame.scopes {
                mark_val(*scope, &mut worklist, &mut live);
            }
            if let Some(env) = frame.env_obj {
                mark_val(env, &mut worklist, &mut live);
            }
            if let Some(args_val) = frame.arguments {
                mark_val(args_val, &mut worklist, &mut live);
            }
            if let Some(ref raw_args) = frame.raw_args {
                for arg in raw_args {
                    mark_val(*arg, &mut worklist, &mut live);
                }
            }
            if let Some(nt) = frame.new_target {
                mark_val(nt, &mut worklist, &mut live);
            }
            if let Some(ho) = frame.home_object {
                mark_val(ho, &mut worklist, &mut live);
            }
            if let Some(f) = &frame.callee_func {
                for uv in &f.upvalues {
                    mark_val(*uv, &mut worklist, &mut live);
                }
                if let Some(ho) = f.home_object {
                    mark_val(ho, &mut worklist, &mut live);
                }
            }
        }
        for val in self.globals.values() {
            mark_val(*val, &mut worklist, &mut live);
        }
        for val in self.modules.values() {
            mark_val(*val, &mut worklist, &mut live);
        }
        for val in self.current_exports.values() {
            mark_val(*val, &mut worklist, &mut live);
        }
        for timer in self.timers.values() {
            mark_val(timer.callback, &mut worklist, &mut live);
            for arg in &timer.args {
                mark_val(*arg, &mut worklist, &mut live);
            }
        }
        for (callee, args) in &self.microtasks {
            mark_val(*callee, &mut worklist, &mut live);
            for arg in args {
                mark_val(*arg, &mut worklist, &mut live);
            }
        }
        if let Some(s) = self.symbol_iterator {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_async_iterator {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_to_string_tag {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_is_concat_spreadable {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_to_primitive {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_species {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_has_instance {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_match {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_match_all {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_replace {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_search {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_split {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_unscopables {
            mark_val(s, &mut worklist, &mut live);
        }
        for s in self.symbol_registry.values() {
            mark_val(*s, &mut worklist, &mut live);
        }

        // 2. Trace transitively
        while let Some(ptr) = worklist.pop() {
            let obj = unsafe { &mut *ptr };
            obj.visit_values_mut(|val| {
                mark_val(*val, &mut worklist, &mut live);
            });
        }

        // 3. Sweep dead objects
        let before_count = self.objects.len();
        let mut i = 0;
        while i < self.objects.len() {
            let ptr = self.objects[i];
            if !live.contains(&ptr) {
                unsafe {
                    std::ptr::drop_in_place(ptr);
                    if !self.heap.contains(ptr as *const u8) {
                        drop(Box::from_raw(ptr));
                    }
                }
                self.objects.swap_remove(i);
            } else {
                i += 1;
            }
        }

        self.heap.nursery.sweep_marked();
        crate::jit::NURSERY_PTRS.with(|cell| {
            let ptrs = unsafe { &mut *cell.get() };
            if !ptrs.bump.is_null() {
                ptrs.bump = self.heap.nursery.cursor();
                ptrs.limit = self.heap.nursery.limit();
            }
        });
        crate::jit::clear_jit_call_cache();
        crate::jit::clear_jit_array_cache();
        before_count - self.objects.len()
    }

    /// Perform a generational minor GC (in-place non-moving mark and sweep).
    pub fn scavenge_nursery(&mut self) {
        self.gc_collect();
    }
}
