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

        let mark_val = |val: JsValue, worklist: &mut Vec<*mut JsObject>, live: &mut HashSet<*mut JsObject>| {
            if let Some(ptr) = val.as_heap_ptr() {
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
        if let Some(s) = self.symbol_species {
            mark_val(s, &mut worklist, &mut live);
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
        if let Some(s) = self.symbol_to_string_tag {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_is_concat_spreadable {
            mark_val(s, &mut worklist, &mut live);
        }
        if let Some(s) = self.symbol_to_primitive {
            mark_val(s, &mut worklist, &mut live);
        }
        for s in self.symbol_registry.values() {
            mark_val(*s, &mut worklist, &mut live);
        }

        // 2. Trace transitively
        while let Some(ptr) = worklist.pop() {
            let obj = unsafe { &*ptr };
            if let Some(proto) = obj.prototype {
                mark_val(proto, &mut worklist, &mut live);
            }
            for slot in &obj.slots {
                mark_val(*slot, &mut worklist, &mut live);
            }
            if let Some(accs) = &obj.accessors {
                for acc in accs.values() {
                    if let Some(g) = acc.getter {
                        mark_val(g, &mut worklist, &mut live);
                    }
                    if let Some(s) = acc.setter {
                        mark_val(s, &mut worklist, &mut live);
                    }
                }
            }
            if let Some(syms) = &obj.symbol_properties {
                for (sym_key, val) in syms.iter() {
                    mark_val(*sym_key, &mut worklist, &mut live);
                    mark_val(*val, &mut worklist, &mut live);
                }
            }
            if let Some(sym_accs) = &obj.symbol_accessors {
                for (sym_key, acc) in sym_accs.iter() {
                    mark_val(*sym_key, &mut worklist, &mut live);
                    if let Some(g) = acc.getter {
                        mark_val(g, &mut worklist, &mut live);
                    }
                    if let Some(s) = acc.setter {
                        mark_val(s, &mut worklist, &mut live);
                    }
                }
            }
            match &obj.kind {
                ObjectKind::Array(elements) => {
                    for el in elements {
                        mark_val(*el, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Proxy { target, handler } => {
                    mark_val(*target, &mut worklist, &mut live);
                    mark_val(*handler, &mut worklist, &mut live);
                }
                ObjectKind::WeakRef(target) => {
                    mark_val(*target, &mut worklist, &mut live);
                }
                ObjectKind::Function(f) => {
                    for uv in &f.upvalues {
                        mark_val(*uv, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Promise(p) => {
                    mark_val(p.result, &mut worklist, &mut live);
                    for r in &p.fulfill_reactions {
                        mark_val(*r, &mut worklist, &mut live);
                    }
                    for r in &p.reject_reactions {
                        mark_val(*r, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Generator(g) => {
                    for v in &g.stack {
                        mark_val(*v, &mut worklist, &mut live);
                    }
                    for uv in &g.function.upvalues {
                        mark_val(*uv, &mut worklist, &mut live);
                    }
                    mark_val(g.this_val, &mut worklist, &mut live);
                    if let Some(args) = g.arguments {
                        mark_val(args, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Map(entries) => {
                    for (k, v) in entries {
                        mark_val(*k, &mut worklist, &mut live);
                        mark_val(*v, &mut worklist, &mut live);
                    }
                }
                ObjectKind::Set(entries) => {
                    for v in entries {
                        mark_val(*v, &mut worklist, &mut live);
                    }
                }
                ObjectKind::WeakMap(entries) => {
                    for (k, v) in entries {
                        mark_val(*k, &mut worklist, &mut live);
                        mark_val(*v, &mut worklist, &mut live);
                    }
                }
                ObjectKind::WeakSet(entries) => {
                    for v in entries {
                        mark_val(*v, &mut worklist, &mut live);
                    }
                }
                ObjectKind::FinalizationRegistry { callback, entries } => {
                    mark_val(*callback, &mut worklist, &mut live);
                    for (t, h, unreg) in entries {
                        mark_val(*t, &mut worklist, &mut live);
                        mark_val(*h, &mut worklist, &mut live);
                        if let Some(u) = unreg {
                            mark_val(*u, &mut worklist, &mut live);
                        }
                    }
                }
                _ => {}
            }
        }

        // 3. Sweep dead objects
        let before_count = self.objects.len();
        let mut i = 0;
        while i < self.objects.len() {
            let ptr = self.objects[i];
            if !live.contains(&ptr) {
                unsafe {
                    drop(Box::from_raw(ptr));
                }
                self.objects.swap_remove(i);
            } else {
                i += 1;
            }
        }

        before_count - self.objects.len()
    }
}
