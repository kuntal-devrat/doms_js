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
    pub fn allocate_js_object(&mut self, obj: JsObject) -> *mut JsObject {
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

        // 2. Nursery full: trigger in-place collection
        self.gc_collect();

        // 3. Retry nursery allocation after collection
        if let Some(ptr) = self.heap.nursery.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }

        // 4. If nursery is still full, allocate in OldGen
        if let Some(ptr) = self.heap.old_gen.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }

        // 4b. OldGen is full. The collection in step 2 just set fresh mark
        //     bits for every live object, so dead blocks can be reclaimed
        //     safely and reused. Never sweep when collection was suppressed:
        //     live objects would not be marked and would be reclaimed.
        if !self.gc_suppressed {
            self.heap.old_gen.sweep();
            if let Some(ptr) = self.heap.old_gen.allocate(payload_size, doms_gc::header::TypeTag::Object) {
                let obj_ptr = ptr as *mut JsObject;
                unsafe {
                    std::ptr::write(obj_ptr, obj);
                }
                self.track_ptr(obj_ptr);
                return obj_ptr;
            }
        }

        // 5. Fallback to large object space
        if let Some(ptr) = self.heap.large_objects.allocate(payload_size, doms_gc::header::TypeTag::Object) {
            let obj_ptr = ptr as *mut JsObject;
            unsafe {
                std::ptr::write(obj_ptr, obj);
            }
            self.track_ptr(obj_ptr);
            return obj_ptr;
        }

        // 6. Emergency fallback to OS heap if all GC spaces are exhausted
        self.oom = true;
        let ptr = Box::into_raw(Box::new(obj));
        self.track_ptr(ptr);
        ptr
    }
}

