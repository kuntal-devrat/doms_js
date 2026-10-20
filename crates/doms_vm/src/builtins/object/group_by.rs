//! `Object.groupBy`.

use super::*;

pub(super) fn register_group_by(vm: &mut Vm) -> JsValue {
    vm.alloc_native_function("groupBy", 2, |vm, _this, args| {
        let items = args.first().copied().unwrap_or(JsValue::undefined());
        let callback = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Object.groupBy callback must be callable".into()));
        }
        let out_obj = vm.alloc_object();
        let len_val = vm.get_property(items, "length");
        let raw_len = len_val.to_number().unwrap_or(0.0);
        if raw_len > 4_294_967_295.0 || raw_len < 0.0 || raw_len.is_infinite() {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        let len = raw_len as usize;
        if len > 100_000 {
            return Err(VmError::RangeError("Invalid array length".into()));
        }
        for i in 0..len {
            let item = vm.get_element(items, i);
            let key_val =
                vm.call(callback, JsValue::undefined(), &[item, JsValue::from_int32(i as i32)])?;
            let key_str = vm.to_string_value(key_val);
            let existing = vm.get_property(out_obj, &key_str);
            if existing.is_undefined() {
                let arr = vm.alloc_array(vec![item]);
                vm.set_property(out_obj, &key_str, arr);
            } else if let Some(ptr) = existing.as_object_ptr() {
                let arr_obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Array(ref mut elems) = arr_obj.kind {
                    elems.push(item);
                }
            }
        }
        Ok(out_obj)
    })
}
