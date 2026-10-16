//! `SharedArrayBuffer` and `ArrayBuffer` builtins.

use super::*;

pub(super) fn register_buffers(vm: &mut Vm) {
    let sab_ctor = vm.alloc_native_function("SharedArrayBuffer", 1, |vm, _this, args| {
        let byte_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if !(0..=64 * 1024 * 1024).contains(&byte_len) {
            return Err(VmError::RangeError("Invalid SharedArrayBuffer length".into()));
        }
        let sab = doms_parallel::SharedArrayBuffer::try_new(byte_len as usize)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        Ok(vm.alloc_shared_array_buffer(Arc::new(sab)))
    });
    let sab_proto = vm.alloc_object();
    vm.set_property(sab_ctor, "prototype", sab_proto);
    vm.globals.insert("SharedArrayBuffer".to_string(), sab_ctor);

    let ab_ctor = vm.alloc_native_function("ArrayBuffer", 1, |vm, _this, args| {
        let byte_len = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if !(0..=64 * 1024 * 1024).contains(&byte_len) {
            return Err(VmError::RangeError("Invalid ArrayBuffer length".into()));
        }
        let sab = doms_parallel::SharedArrayBuffer::try_new(byte_len as usize)
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        Ok(vm.alloc_array_buffer(Arc::new(sab)))
    });
    let ab_is_view = vm.alloc_native_function("isView", 1, |_vm, _this, args| {
        if let Some(arg) = args.first() {
            if let Some(ptr) = arg.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if matches!(obj.kind, ObjectKind::TypedArray(_) | ObjectKind::DataView { .. }) {
                    return Ok(JsValue::from_boolean(true));
                }
            }
        }
        Ok(JsValue::from_boolean(false))
    });
    vm.set_property(ab_ctor, "isView", ab_is_view);

    let ab_proto = vm.alloc_object();
    let ab_slice = vm.alloc_native_function("slice", 2, |vm, this, args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::ArrayBuffer(buf) = &obj.kind {
                let len = buf.byte_length() as i32;
                let start = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
                let end = args.get(1).and_then(|v| v.to_int32()).unwrap_or(len);
                let s = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
                let e = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
                let count = if e > s { e - s } else { 0 };
                let new_buf = doms_parallel::SharedArrayBuffer::try_new(count)
                    .map_err(|e| VmError::RuntimeError(e.into()))?;
                for i in 0..count {
                    if let Some(b) = buf.get(s + i) {
                        new_buf.set(i, b);
                    }
                }
                return Ok(vm.alloc_array_buffer(Arc::new(new_buf)));
            }
        }
        Err(VmError::TypeError("ArrayBuffer.prototype.slice called on non-ArrayBuffer".into()))
    });
    vm.set_property(ab_proto, "slice", ab_slice);
    vm.set_property(ab_ctor, "prototype", ab_proto);
    vm.globals.insert("ArrayBuffer".to_string(), ab_ctor);
}
