//! Allocation helpers for every JavaScript object kind.

use super::*;

mod collections;
mod functions;
mod misc;
mod objects;
mod typed_arrays;

impl Vm {
    /// Allocate a `JsObject` directly in OldGen (immovable tenured space for functions/prototypes).
    pub fn allocate_js_object_old_gen(&mut self, obj: JsObject) -> *mut JsObject {
        let payload_size = std::mem::size_of::<JsObject>();
        if let Some(ptr) = self.heap.old_gen.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }
        self.allocate_js_object(obj)
    }

    /// Allocate a `JsObject` using the GC heap bump nursery and fallback spaces.
    pub fn allocate_js_object(&mut self, mut obj: JsObject) -> *mut JsObject {
        let payload_size = std::mem::size_of::<JsObject>();

        // 1. Try fast bump allocation in the young generation nursery
        if let Some(ptr) = self.heap.nursery.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }

        // 2. Nursery full: protect interior references before scavenge
        let prev_temp = self.temp_roots.len();
        obj.visit_values_mut(|val| self.temp_roots.push(*val));

        // 3. Trigger minor GC (nursery scavenge)
        self.scavenge_nursery();

        // 4. Restore updated references from temp_roots
        {
            let mut iter = self.temp_roots.drain(prev_temp..);
            obj.visit_values_mut(|val| {
                if let Some(updated) = iter.next() {
                    *val = updated;
                }
            });
        }

        // 5. Retry nursery allocation after scavenge
        if let Some(ptr) = self.heap.nursery.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }

        // 6. If nursery is still full, allocate in OldGen
        if let Some(ptr) = self.heap.old_gen.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }

        // 7. OldGen full: trigger full mark-sweep major GC and retry OldGen
        let prev_temp2 = self.temp_roots.len();
        obj.visit_values_mut(|val| self.temp_roots.push(*val));
        self.gc_collect();
        {
            let mut iter = self.temp_roots.drain(prev_temp2..);
            obj.visit_values_mut(|val| {
                if let Some(updated) = iter.next() {
                    *val = updated;
                }
            });
        }

        if let Some(ptr) = self.heap.old_gen.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }

        // 8. Fallback to large object space
        if let Some(ptr) = self.heap.large_objects.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }

        // 9. Emergency fallback to OS heap if all GC spaces are exhausted
        self.oom = true;
        let ptr = Box::into_raw(Box::new(obj));
        self.track_ptr(ptr);
        ptr
    }
}

