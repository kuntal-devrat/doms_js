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

    let sab_byte_len_getter = vm.alloc_native_function("get byteLength", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::SharedArrayBuffer(buf) = &obj.kind {
                return Ok(JsValue::from_double(buf.byte_length() as f64));
            }
        }
        Err(VmError::TypeError("SharedArrayBuffer.prototype.byteLength called on non-SharedArrayBuffer".into()))
    });
    if let Some(ptr) = sab_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor("byteLength".to_string(), Some(sab_byte_len_getter), None);
    }
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("SharedArrayBuffer".into());
        vm.set_symbol_property(sab_proto, sym_tag, tag_val);
        if let Some(ptr) = sab_proto.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.set_symbol_attributes(sym_tag, PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: true,
            });
        }
    }

    vm.set_property(sab_ctor, "prototype", sab_proto);
    vm.set_property_non_writable(sab_ctor, "prototype".to_string());
    vm.set_property_non_enumerable(sab_ctor, "prototype".to_string());
    vm.set_property_non_configurable(sab_ctor, "prototype".to_string());
    helpers::add_species_getter(vm, sab_ctor);
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
    vm.set_property_non_enumerable(ab_ctor, "isView".to_string());

    let ab_proto = vm.alloc_object();

    let ab_byte_len_getter = vm.alloc_native_function("get byteLength", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::ArrayBuffer(buf) = &obj.kind {
                if buf.is_detached() {
                    return Err(VmError::TypeError("ArrayBuffer is detached".into()));
                }
                return Ok(JsValue::from_double(buf.byte_length() as f64));
            }
        }
        Err(VmError::TypeError("ArrayBuffer.prototype.byteLength called on non-ArrayBuffer".into()))
    });
    if let Some(ptr) = ab_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor("byteLength".to_string(), Some(ab_byte_len_getter), None);
    }

    let ab_slice = vm.alloc_native_function("slice", 2, |vm, this, args| {
        let (buf, len) = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::ArrayBuffer(buf) = &obj.kind {
                if buf.is_detached() {
                    return Err(VmError::TypeError("ArrayBuffer is detached".into()));
                }
                (Arc::clone(buf), buf.byte_length())
            } else {
                return Err(VmError::TypeError("ArrayBuffer.prototype.slice called on non-ArrayBuffer".into()));
            }
        } else {
            return Err(VmError::TypeError("ArrayBuffer.prototype.slice called on non-ArrayBuffer".into()));
        };

        // 6. Let relativeStart = ? ToIntegerOrInfinity(start).
        let start_val = args.first().copied().unwrap_or(JsValue::undefined());
        let relative_start = vm.to_number_value(start_val)?.trunc();
        let first = if relative_start.is_infinite() && relative_start < 0.0 {
            0
        } else if relative_start < 0.0 {
            (len as f64 + relative_start).max(0.0) as usize
        } else {
            (relative_start.min(len as f64)).max(0.0) as usize
        };

        // 8. If end is undefined, let relativeEnd = len; else let relativeEnd = ? ToIntegerOrInfinity(end).
        let end_val = args.get(1).copied().unwrap_or(JsValue::undefined());
        let final_idx = if end_val.is_undefined() {
            len
        } else {
            let relative_end = vm.to_number_value(end_val)?.trunc();
            if relative_end.is_infinite() && relative_end < 0.0 {
                0
            } else if relative_end < 0.0 {
                (len as f64 + relative_end).max(0.0) as usize
            } else {
                (relative_end.min(len as f64)).max(0.0) as usize
            }
        };

        // 10. Let newLen = max(final - first, 0).
        let new_len = if final_idx > first { final_idx - first } else { 0 };

        // 11. Let ctor = ? SpeciesConstructor(O, %ArrayBuffer%).
        let default_ctor = vm.globals.get("ArrayBuffer").copied().unwrap_or(JsValue::undefined());
        let ctor = vm.species_constructor(this, default_ctor)?;

        // 12. Let new = ? Construct(ctor, « 𝔽(newLen) »).
        let new_obj = vm.construct(ctor, &[JsValue::from_double(new_len as f64)])?;

        // 13. Perform ? RequireInternalSlot(new, [[ArrayBufferData]]).
        // 14. If IsSharedArrayBuffer(new) is true, throw a TypeError exception.
        // 15. If IsDetachedBuffer(new) is true, throw a TypeError exception.
        // 16. If SameValue(new, O) is true, throw a TypeError exception.
        // 17. If new.[[ArrayByteLength]] < newLen, throw a TypeError exception.
        let new_buf = if let Some(n_ptr) = new_obj.as_object_ptr() {
            if new_obj == this {
                return Err(VmError::TypeError("Species constructor returned same ArrayBuffer".into()));
            }
            let n_obj = unsafe { &*(n_ptr as *mut JsObject) };
            if let ObjectKind::ArrayBuffer(nb) = &n_obj.kind {
                if nb.is_detached() {
                    return Err(VmError::TypeError("Species constructor returned detached ArrayBuffer".into()));
                }
                if nb.byte_length() < new_len {
                    return Err(VmError::TypeError("Species constructor returned smaller ArrayBuffer".into()));
                }
                Arc::clone(nb)
            } else {
                return Err(VmError::TypeError("Species constructor did not return an ArrayBuffer".into()));
            }
        } else {
            return Err(VmError::TypeError("Species constructor did not return an object".into()));
        };

        // 19. If IsDetachedBuffer(O) is true, throw a TypeError exception.
        if buf.is_detached() {
            return Err(VmError::TypeError("Source ArrayBuffer was detached during species construct".into()));
        }

        for i in 0..new_len {
            if let Some(b) = buf.get(first + i) {
                new_buf.set(i, b);
            }
        }

        Ok(new_obj)
    });

    vm.set_property(ab_proto, "slice", ab_slice);
    vm.set_property_non_enumerable(ab_proto, "slice".to_string());

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("ArrayBuffer".into());
        vm.set_symbol_property(ab_proto, sym_tag, tag_val);
        if let Some(ptr) = ab_proto.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.set_symbol_attributes(sym_tag, PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: true,
            });
        }
    }

    vm.set_property(ab_ctor, "prototype", ab_proto);
    vm.set_property_non_writable(ab_ctor, "prototype".to_string());
    vm.set_property_non_enumerable(ab_ctor, "prototype".to_string());
    vm.set_property_non_configurable(ab_ctor, "prototype".to_string());
    helpers::add_species_getter(vm, ab_ctor);
    vm.globals.insert("ArrayBuffer".to_string(), ab_ctor);
}
