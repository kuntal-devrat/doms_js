//! Object tracking and tracing garbage collection entry points.

use super::*;

impl Vm {
    // ── Allocation Helpers ──

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

        let to_start = self.heap.nursery.to_start() as usize;
        let space_size = self.heap.nursery.space_size();
        let mark_val = |val: JsValue, worklist: &mut Vec<*mut JsObject>, live: &mut HashSet<*mut JsObject>| {
            if let Some(ptr) = val.as_heap_ptr() {
                let addr = ptr as usize;
                if addr >= to_start && addr < to_start + space_size {
                    panic!("Corrupted reference in GC: pointer {:p} is in old from-space (to-space)!", ptr);
                }
                let obj_ptr = ptr as *mut JsObject;
                if live.insert(obj_ptr) {
                    worklist.push(obj_ptr);
                }
            }
        };

        // 1. Mark Roots
        if let Some(pending) = self.pending_alloc {
            if live.insert(pending) {
                worklist.push(pending);
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
        for (target, _) in &self.non_configurable_props {
            mark_val(*target, &mut worklist, &mut live);
        }
        for (target, _) in &self.non_writable_props {
            mark_val(*target, &mut worklist, &mut live);
        }
        for (target, _) in &self.non_enumerable_props {
            mark_val(*target, &mut worklist, &mut live);
        }
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

        before_count - self.objects.len()
    }

    /// Perform a generational minor GC (nursery scavenge) using Cheney's copying algorithm.
    pub fn scavenge_nursery(&mut self) {
        let mut to_cursor = self.heap.nursery.to_start();
        let to_limit = self.heap.nursery.to_limit();
        let to_start = to_cursor;

        // Helper: copy an object if it is in nursery from-space, and update JsValue in-place
        let copy_val = |val: &mut JsValue, to_cursor: &mut *mut u8| {
            if let Some(ptr) = val.as_heap_ptr() {
                if self.heap.nursery.contains(ptr) {
                    let new_ptr = self.heap.nursery.copy_object(ptr, to_cursor, to_limit);
                    unsafe {
                        val.update_heap_ptr(new_ptr);
                    }
                }
            }
        };

        // ── Phase 1: Evacuate root-reachable objects to to-space ──
        if let Some(pending) = self.pending_alloc {
            let ptr = pending as *mut u8;
            if self.heap.nursery.contains(ptr) {
                let new_ptr = self.heap.nursery.copy_object(ptr, &mut to_cursor, to_limit);
                self.pending_alloc = Some(new_ptr as *mut JsObject);
            }
        }
        if let Some(mut proto) = self.object_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.object_proto = Some(proto);
        }
        if let Some(mut proto) = self.array_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.array_proto = Some(proto);
        }
        if let Some(mut proto) = self.function_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.function_proto = Some(proto);
        }
        if let Some(mut proto) = self.regexp_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.regexp_proto = Some(proto);
        }
        if let Some(mut proto) = self.string_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.string_proto = Some(proto);
        }
        if let Some(mut proto) = self.number_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.number_proto = Some(proto);
        }
        if let Some(mut proto) = self.boolean_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.boolean_proto = Some(proto);
        }
        if let Some(mut proto) = self.symbol_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.symbol_proto = Some(proto);
        }
        if let Some(mut proto) = self.promise_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.promise_proto = Some(proto);
        }
        if let Some(mut proto) = self.date_proto {
            copy_val(&mut proto, &mut to_cursor);
            self.date_proto = Some(proto);
        }
        if let Some(mut s) = self.symbol_iterator {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_iterator = Some(s);
        }
        if let Some(mut s) = self.symbol_async_iterator {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_async_iterator = Some(s);
        }
        if let Some(mut s) = self.symbol_to_string_tag {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_to_string_tag = Some(s);
        }
        if let Some(mut s) = self.symbol_is_concat_spreadable {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_is_concat_spreadable = Some(s);
        }
        if let Some(mut s) = self.symbol_species {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_species = Some(s);
        }
        if let Some(mut s) = self.symbol_to_primitive {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_to_primitive = Some(s);
        }
        if let Some(mut s) = self.symbol_has_instance {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_has_instance = Some(s);
        }
        if let Some(mut s) = self.symbol_match {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_match = Some(s);
        }
        if let Some(mut s) = self.symbol_match_all {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_match_all = Some(s);
        }
        if let Some(mut s) = self.symbol_replace {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_replace = Some(s);
        }
        if let Some(mut s) = self.symbol_search {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_search = Some(s);
        }
        if let Some(mut s) = self.symbol_split {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_split = Some(s);
        }
        if let Some(mut s) = self.symbol_unscopables {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_unscopables = Some(s);
        }
        for s in self.symbol_registry.values_mut() {
            copy_val(s, &mut to_cursor);
        }
        for val in self.global_lexicals.values_mut() {
            copy_val(val, &mut to_cursor);
        }
        for val in &mut self.temp_roots {
            copy_val(val, &mut to_cursor);
        }
        for val in &mut self.stack {
            copy_val(val, &mut to_cursor);
        }
        copy_val(&mut self.global_object, &mut to_cursor);
        for frame in &mut self.frames {
            if let Some(mut gv) = frame.gen_val {
                copy_val(&mut gv, &mut to_cursor);
                frame.gen_val = Some(gv);
            }
            if let Some(mut cv) = frame.callee_val {
                copy_val(&mut cv, &mut to_cursor);
                frame.callee_val = Some(cv);
            }
            copy_val(&mut frame.this_val, &mut to_cursor);
            for local in &mut frame.locals {
                copy_val(local, &mut to_cursor);
            }
            for scope in &mut frame.scopes {
                copy_val(scope, &mut to_cursor);
            }
            if let Some(mut env) = frame.env_obj {
                copy_val(&mut env, &mut to_cursor);
                frame.env_obj = Some(env);
            }
            if let Some(mut args_val) = frame.arguments {
                copy_val(&mut args_val, &mut to_cursor);
                frame.arguments = Some(args_val);
            }
            if let Some(mut nt) = frame.new_target {
                copy_val(&mut nt, &mut to_cursor);
                frame.new_target = Some(nt);
            }
            if let Some(mut ho) = frame.home_object {
                copy_val(&mut ho, &mut to_cursor);
                frame.home_object = Some(ho);
            }
            if let Some(f) = &mut frame.callee_func {
                for uv in &mut f.upvalues {
                    copy_val(uv, &mut to_cursor);
                }
                if let Some(ref mut ho) = f.home_object {
                    copy_val(ho, &mut to_cursor);
                }
            }
        }
        for val in self.globals.values_mut() {
            copy_val(val, &mut to_cursor);
        }
        for val in self.modules.values_mut() {
            copy_val(val, &mut to_cursor);
        }
        for val in self.current_exports.values_mut() {
            copy_val(val, &mut to_cursor);
        }
        for timer in self.timers.values_mut() {
            copy_val(&mut timer.callback, &mut to_cursor);
            for arg in &mut timer.args {
                copy_val(arg, &mut to_cursor);
            }
        }
        for (callee, args) in &mut self.microtasks {
            copy_val(callee, &mut to_cursor);
            for arg in args {
                copy_val(arg, &mut to_cursor);
            }
        }
        if let Some(mut s) = self.symbol_iterator {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_iterator = Some(s);
        }
        if let Some(mut s) = self.symbol_async_iterator {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_async_iterator = Some(s);
        }
        if let Some(mut s) = self.symbol_to_string_tag {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_to_string_tag = Some(s);
        }
        if let Some(mut s) = self.symbol_is_concat_spreadable {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_is_concat_spreadable = Some(s);
        }
        if let Some(mut s) = self.symbol_to_primitive {
            copy_val(&mut s, &mut to_cursor);
            self.symbol_to_primitive = Some(s);
        }
        for s in self.symbol_registry.values_mut() {
            copy_val(s, &mut to_cursor);
        }

        // Rehash sets/maps keyed by JsValue
        let mut update_set = |set: &mut HashSet<(JsValue, String)>| {
            let mut updated = HashSet::with_capacity(set.len());
            for (mut target, key) in set.drain() {
                copy_val(&mut target, &mut to_cursor);
                updated.insert((target, key));
            }
            *set = updated;
        };
        update_set(&mut self.non_configurable_props);
        update_set(&mut self.non_writable_props);
        update_set(&mut self.non_enumerable_props);
        update_set(&mut self.deleted_props);

        let mut updated_non_ext = HashSet::with_capacity(self.non_extensible_objects.len());
        for mut target in self.non_extensible_objects.drain() {
            copy_val(&mut target, &mut to_cursor);
            updated_non_ext.insert(target);
        }
        self.non_extensible_objects = updated_non_ext;

        let mut updated_ns = HashSet::with_capacity(self.module_namespace_objects.len());
        for mut target in self.module_namespace_objects.drain() {
            copy_val(&mut target, &mut to_cursor);
            updated_ns.insert(target);
        }
        self.module_namespace_objects = updated_ns;

        if let Some(mut cmo) = self.current_module_obj {
            copy_val(&mut cmo, &mut to_cursor);
            self.current_module_obj = Some(cmo);
        }

        for (target, _) in self.imported_bindings.values_mut() {
            copy_val(target, &mut to_cursor);
        }
        for target in self.star_exports.values_mut() {
            copy_val(target, &mut to_cursor);
        }
        let mut updated_reexports = HashMap::with_capacity(self.module_reexports.len());
        for ((mut k_tgt, k_str), (mut v_tgt, v_str)) in self.module_reexports.drain() {
            copy_val(&mut k_tgt, &mut to_cursor);
            copy_val(&mut v_tgt, &mut to_cursor);
            updated_reexports.insert((k_tgt, k_str), (v_tgt, v_str));
        }
        self.module_reexports = updated_reexports;

        // Evacuate references from objects outside the nursery (e.g. OldGen or large objects)
        for &ptr in &self.objects {
            if !self.heap.nursery.contains(ptr as *const u8) {
                let obj = unsafe { &mut *ptr };
                obj.visit_values_mut(|val| copy_val(val, &mut to_cursor));
            }
        }

        // ── Phase 2: Cheney scan of to-space (transitive closure) ──
        let mut scan = to_start;
        while (scan as usize) < (to_cursor as usize) {
            let header = unsafe { &*(scan as *const doms_gc::GcHeader) };
            let total_size = header.total_size();
            let payload = unsafe { scan.add(doms_gc::header::HEADER_SIZE) as *mut JsObject };
            let obj = unsafe { &mut *payload };

            obj.visit_values_mut(|val| copy_val(val, &mut to_cursor));

            scan = unsafe { scan.add(total_size) };
        }

        // ── Phase 3: Walk self.objects: drop dead objects in from-space, update live pointers ──
        let mut i = 0;
        while i < self.objects.len() {
            let ptr = self.objects[i];
            if self.heap.nursery.contains(ptr as *const u8) {
                let header = unsafe { &*((ptr as *mut doms_gc::GcHeader).sub(1)) };
                if header.is_forwarded() {
                    self.objects[i] = unsafe { header.forwarding_address() as *mut JsObject };
                    i += 1;
                } else {
                    unsafe {
                        std::ptr::drop_in_place(ptr);
                    }
                    self.objects.swap_remove(i);
                }
            } else {
                i += 1;
            }
        }

        // ── Phase 4: Finish scavenge (swaps spaces, resets cursor, clears old from-space) ──
        self.heap.nursery.finish_scavenge(to_cursor);
    }
}

