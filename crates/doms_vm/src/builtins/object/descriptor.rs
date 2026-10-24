//! `Object.getOwnPropertyDescriptor` and `Object.getOwnPropertyDescriptors`.

use super::names::get_own_property_keys;
use super::*;

/// Evaluates internal [[GetOwnProperty]] according to ECMAScript specification.
/// If `target` is a Proxy, invokes the `getOwnPropertyDescriptor` trap if present.
pub fn get_own_property_descriptor_internal(
    vm: &mut Vm,
    target: JsValue,
    prop_key: &PropertyKeyVal,
) -> Result<JsValue, VmError> {
    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if matches!(obj.kind, ObjectKind::Proxy { .. }) {
            return vm.proxy_get_own_property_descriptor(target, prop_key);
        }
    }

    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                let sym = *sym;
                if let Some(accs) = &obj.symbol_accessors {
                    if let Some(acc) = accs.get(&sym) {
                        let desc = vm.alloc_object();
                        vm.set_property(desc, "get", acc.getter.unwrap_or(JsValue::undefined()));
                        vm.set_property(desc, "set", acc.setter.unwrap_or(JsValue::undefined()));
                        let is_enum = acc.enumerable;
                        let is_conf = acc.configurable;
                        vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                        vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                        return Ok(desc);
                    }
                }
                if let Some(syms) = &obj.symbol_properties {
                    if let Some(val) = syms.get(&sym) {
                        let desc = vm.alloc_object();
                        vm.set_property(desc, "value", *val);
                        if let Some(attrs) =
                            obj.symbol_attributes.as_ref().and_then(|m| m.get(&sym))
                        {
                            vm.set_property(
                                desc,
                                "writable",
                                JsValue::from_boolean(attrs.writable),
                            );
                            vm.set_property(
                                desc,
                                "enumerable",
                                JsValue::from_boolean(attrs.enumerable),
                            );
                            vm.set_property(
                                desc,
                                "configurable",
                                JsValue::from_boolean(attrs.configurable),
                            );
                        } else {
                            let is_tag = vm.symbol_to_string_tag.is_some_and(|t| t == sym);
                            let is_iter = vm.symbol_iterator.is_some_and(|i| i == sym);
                            let is_writ = !is_tag;
                            let is_enum = !is_tag && !is_iter;
                            let is_conf = !vm.module_namespace_objects.contains(&target);
                            vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                        }
                        return Ok(desc);
                    }
                }
            }
            PropertyKeyVal::String(prop_name) => {
                if let Some(accs) = &obj.accessors {
                    if let Some(acc) = accs.get(prop_name) {
                        let desc = vm.alloc_object();
                        vm.set_property(desc, "get", acc.getter.unwrap_or(JsValue::undefined()));
                        vm.set_property(desc, "set", acc.setter.unwrap_or(JsValue::undefined()));
                        let is_enum = acc.enumerable;
                        vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                        let is_conf = acc.configurable;
                        vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                        return Ok(desc);
                    }
                }
                match &obj.kind {
                    ObjectKind::Array(elements) => {
                        if prop_name == "length" {
                            let desc = vm.alloc_object();
                            let is_writ = obj.is_array_length_writable();
                            let len_val = if let Some(info) =
                                vm.shapes.get_property_info(obj.shape_id, "length")
                            {
                                if let Some(val) = obj.get_slot(info.offset) {
                                    val
                                } else {
                                    JsValue::from_double(elements.len() as f64)
                                }
                            } else {
                                let len = elements.len();
                                if len <= i32::MAX as usize {
                                    JsValue::from_int32(len as i32)
                                } else {
                                    JsValue::from_double(len as f64)
                                }
                            };
                            vm.set_property(desc, "value", len_val);
                            vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(false));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                            return Ok(desc);
                        }
                        if let Ok(idx) = prop_name.parse::<usize>() {
                            if let Some(elem) = elements.get(idx) {
                                if !elem.is_hole() {
                                    let desc = vm.alloc_object();
                                    vm.set_property(desc, "value", *elem);
                                    let (is_writ, is_enum, is_conf) = if let Some(info) =
                                        vm.shapes.get_property_info(obj.shape_id, prop_name)
                                    {
                                        (
                                            info.attributes.writable,
                                            info.attributes.enumerable,
                                            info.attributes.configurable,
                                        )
                                    } else {
                                        (true, true, true)
                                    };
                                    vm.set_property(
                                        desc,
                                        "writable",
                                        JsValue::from_boolean(is_writ),
                                    );
                                    vm.set_property(
                                        desc,
                                        "enumerable",
                                        JsValue::from_boolean(is_enum),
                                    );
                                    vm.set_property(
                                        desc,
                                        "configurable",
                                        JsValue::from_boolean(is_conf),
                                    );
                                    return Ok(desc);
                                }
                            }
                        }
                    }
                    ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                        if prop_name == "length" {
                            let desc = vm.alloc_object();
                            vm.set_property(
                                desc,
                                "value",
                                JsValue::from_int32(s.encode_utf16().count() as i32),
                            );
                            vm.set_property(desc, "writable", JsValue::from_boolean(false));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(false));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                            return Ok(desc);
                        }
                        if let Ok(idx) = prop_name.parse::<usize>() {
                            if let Some(ch) = s.chars().nth(idx) {
                                let ch_val = vm.alloc_string(ch.to_string());
                                let desc = vm.alloc_object();
                                vm.set_property(desc, "value", ch_val);
                                vm.set_property(desc, "writable", JsValue::from_boolean(false));
                                vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                                vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                                return Ok(desc);
                            }
                        }
                    }
                    ObjectKind::TypedArray(ta) => {
                        if let Ok(idx) = prop_name.parse::<usize>() {
                            if idx < ta.length {
                                let val = Vm::read_typed_array_element(ta, idx)
                                    .unwrap_or(JsValue::undefined());
                                let desc = vm.alloc_object();
                                vm.set_property(desc, "value", val);
                                vm.set_property(desc, "writable", JsValue::from_boolean(true));
                                vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                                vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                                return Ok(desc);
                            }
                        }
                    }
                    _ => {}
                }
                if let Some(info) = vm.shapes.get_property_info(obj.shape_id, prop_name) {
                    let val = vm.get_property(target, prop_name);
                    if vm.module_namespace_objects.contains(&target)
                        && val == JsValue::uninitialized()
                    {
                        return Err(VmError::ReferenceError(format!(
                            "Cannot access uninitialized export '{}'",
                            prop_name
                        )));
                    }
                    let desc = vm.alloc_object();
                    vm.set_property(desc, "value", val);
                    let is_writ = if vm.module_namespace_objects.contains(&target) {
                        true
                    } else {
                        info.attributes.writable
                    };
                    let is_enum = if vm.module_namespace_objects.contains(&target) {
                        true
                    } else {
                        info.attributes.enumerable
                    };
                    let is_conf = if vm.module_namespace_objects.contains(&target) {
                        false
                    } else {
                        info.attributes.configurable
                    };
                    vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                    vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                    vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                    return Ok(desc);
                }
                if let Some((tgt_mod, orig_name)) =
                    vm.module_reexports.get(&(target, prop_name.clone())).cloned()
                {
                    let val = vm.get_property(tgt_mod, &orig_name);
                    if val == JsValue::uninitialized() {
                        return Err(VmError::ReferenceError(format!(
                            "Cannot access uninitialized export '{}'",
                            prop_name
                        )));
                    }
                    let desc = vm.alloc_object();
                    vm.set_property(desc, "value", val);
                    vm.set_property(desc, "writable", JsValue::from_boolean(true));
                    vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                    vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                    return Ok(desc);
                }
            }
        }
    }
    Ok(JsValue::undefined())
}

pub(super) fn register_descriptor(vm: &mut Vm) -> (JsValue, JsValue) {
    let get_own_prop_desc_fn =
        vm.alloc_native_function("getOwnPropertyDescriptor", 2, |vm, _this, args| {
            let target = match args.first() {
                Some(t) if !t.is_nullish() => *t,
                _ => {
                    return Err(VmError::TypeError(
                        "Object.getOwnPropertyDescriptor called on non-object".into(),
                    ));
                }
            };
            let target_obj =
                if target.is_js_object() { target } else { vm.to_object_value(target)? };
            let prop_key = match args.get(1) {
                Some(p) => vm.to_property_key(*p)?,
                None => PropertyKeyVal::String("undefined".to_string()),
            };
            get_own_property_descriptor_internal(vm, target_obj, &prop_key)
        });

    let get_own_prop_descriptors_fn =
        vm.alloc_native_function("getOwnPropertyDescriptors", 1, |vm, _this, args| {
            let target = match args.first() {
                Some(t) if !t.is_nullish() => *t,
                _ => {
                    return Err(VmError::TypeError(
                        "Cannot convert undefined or null to object".into(),
                    ));
                }
            };
            let target_obj =
                if target.is_js_object() { target } else { vm.to_object_value(target)? };
            let keys = get_own_property_keys(vm, target_obj)?;
            let descriptors = vm.alloc_object();
            for key in keys {
                let desc = get_own_property_descriptor_internal(vm, target_obj, &key)?;
                if !desc.is_undefined() {
                    match key {
                        PropertyKeyVal::String(s) => vm.set_property(descriptors, &s, desc),
                        PropertyKeyVal::Symbol(sym) => {
                            vm.set_symbol_property(descriptors, sym, desc)
                        }
                    }
                }
            }
            Ok(descriptors)
        });

    (get_own_prop_desc_fn, get_own_prop_descriptors_fn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_own_property_descriptor_internal() {
        let mut vm = Vm::new();

        // 1. Non-object target
        let non_obj = JsValue::from_double(42.0);
        let prop_key = PropertyKeyVal::String("prop".to_string());
        let result = get_own_property_descriptor_internal(&mut vm, non_obj, &prop_key).unwrap();
        assert!(result.is_undefined());

        // 2. Object with a string property
        let obj = vm.alloc_object();
        let val = JsValue::from_double(100.0);
        vm.set_property(obj, "testProp", val);

        let test_prop_key = PropertyKeyVal::String("testProp".to_string());
        let desc = get_own_property_descriptor_internal(&mut vm, obj, &test_prop_key).unwrap();

        assert!(desc.is_js_object());

        // Check descriptor properties
        let desc_value = vm.get_property(desc, "value");
        assert_eq!(desc_value, val);

        let desc_writable = vm.get_property(desc, "writable");
        assert_eq!(desc_writable, JsValue::from_boolean(true));

        let desc_enumerable = vm.get_property(desc, "enumerable");
        assert_eq!(desc_enumerable, JsValue::from_boolean(true));

        let desc_configurable = vm.get_property(desc, "configurable");
        assert_eq!(desc_configurable, JsValue::from_boolean(true));
    }
}
