//! `Object.getOwnPropertyDescriptor` and `Object.getOwnPropertyDescriptors`.

use super::*;
use super::names::get_own_property_keys;

/// Evaluates internal [[GetOwnProperty]] according to ECMAScript specification.
/// If `target` is a Proxy, invokes the `getOwnPropertyDescriptor` trap if present.
pub fn get_own_property_descriptor_internal(
    vm: &mut Vm,
    target: JsValue,
    prop_key: &PropertyKeyVal,
) -> Result<JsValue, VmError> {
    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Proxy { target: inner_target, handler } = &obj.kind {
            let inner_target = *inner_target;
            let handler = *handler;
            let trap = vm.get_property(handler, "getOwnPropertyDescriptor");
            if vm.is_callable(trap) {
                let key_arg = match prop_key {
                    PropertyKeyVal::String(s) => vm.alloc_string(s.clone()),
                    PropertyKeyVal::Symbol(sym) => *sym,
                };
                let trap_res = vm.call(trap, handler, &[inner_target, key_arg])?;
                if trap_res.is_undefined() {
                    return Ok(JsValue::undefined());
                }
                if !trap_res.is_object() {
                    return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor trap must return an object or undefined".into()));
                }
                return Ok(trap_res);
            } else {
                return get_own_property_descriptor_internal(vm, inner_target, prop_key);
            }
        }
    }

    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                let sym = *sym;
                let key_name = format!("[Symbol:{:x}]", sym.raw_bits());
                if let Some(accs) = &obj.symbol_accessors {
                    if let Some(acc) = accs.get(&sym) {
                        let desc = vm.alloc_object();
                        vm.set_property(desc, "get", acc.getter.unwrap_or(JsValue::undefined()));
                        vm.set_property(desc, "set", acc.setter.unwrap_or(JsValue::undefined()));
                        let is_enum = !vm.non_enumerable_props.contains(&(target, key_name.clone()));
                        let is_conf = !vm.non_configurable_props.contains(&(target, key_name));
                        vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                        vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                        return Ok(desc);
                    }
                }
                if let Some(syms) = &obj.symbol_properties {
                    if let Some(val) = syms.get(&sym) {
                        let desc = vm.alloc_object();
                        vm.set_property(desc, "value", *val);
                        let is_tag = vm.symbol_to_string_tag.is_some_and(|t| t == sym);
                        let is_iter = vm.symbol_iterator.is_some_and(|i| i == sym);
                        let is_writ = if vm.non_writable_props.contains(&(target, key_name.clone())) {
                            false
                        } else {
                            !is_tag
                        };
                        let is_enum = if vm.non_enumerable_props.contains(&(target, key_name.clone())) {
                            false
                        } else {
                            !is_tag && !is_iter
                        };
                        let is_conf = if vm.module_namespace_objects.contains(&target) || is_tag || vm.non_configurable_props.contains(&(target, key_name)) {
                            false
                        } else {
                            true
                        };
                        vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                        vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                        vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
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
                        let is_enum = !vm.non_enumerable_props.contains(&(target, prop_name.clone()));
                        vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                        let is_conf = !vm.non_configurable_props.contains(&(target, prop_name.clone()));
                        vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                        return Ok(desc);
                    }
                }
                if vm.deleted_props.contains(&(target, prop_name.clone())) {
                    return Ok(JsValue::undefined());
                }
                match &obj.kind {
                    ObjectKind::Array(elements) => {
                        if prop_name == "length" {
                            let desc = vm.alloc_object();
                            let is_writ = !vm.non_writable_props.contains(&(target, "length".to_string()));
                            vm.set_property(desc, "value", JsValue::from_int32(elements.len() as i32));
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
                                    let is_writ = !vm.non_writable_props.contains(&(target, prop_name.clone()));
                                    let is_enum = !vm.non_enumerable_props.contains(&(target, prop_name.clone()));
                                    let is_conf = !vm.non_configurable_props.contains(&(target, prop_name.clone()));
                                    vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                                    vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                                    vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                                    return Ok(desc);
                                }
                            }
                        }
                    }
                    ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                        if prop_name == "length" {
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "value", JsValue::from_int32(s.encode_utf16().count() as i32));
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
                                let val = Vm::read_typed_array_element(ta, idx).unwrap_or(JsValue::undefined());
                                let desc = vm.alloc_object();
                                vm.set_property(desc, "value", val);
                                vm.set_property(desc, "writable", JsValue::from_boolean(true));
                                vm.set_property(desc, "enumerable", JsValue::from_boolean(true));
                                vm.set_property(desc, "configurable", JsValue::from_boolean(false));
                                return Ok(desc);
                            }
                        }
                    }
                    ObjectKind::Function(_) | ObjectKind::NativeFunction(_) => {
                        if prop_name == "length" {
                            let len = match &obj.kind {
                                ObjectKind::Function(f) => f.params.iter().take_while(|p| !p.starts_with("...")).count(),
                                ObjectKind::NativeFunction(n) => n.length as usize,
                                _ => 0,
                            };
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "value", JsValue::from_int32(len as i32));
                            let is_writ = false;
                            let is_enum = false;
                            let is_conf = !vm.non_configurable_props.contains(&(target, prop_name.clone()));
                            vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                            return Ok(desc);
                        }
                        if prop_name == "name" {
                            let name_str = match &obj.kind {
                                ObjectKind::Function(f) => f.name.as_deref().unwrap_or(""),
                                ObjectKind::NativeFunction(n) => &n.name,
                                _ => "",
                            };
                            let name_val = vm.alloc_string(name_str.to_string());
                            let desc = vm.alloc_object();
                            vm.set_property(desc, "value", name_val);
                            let is_writ = false;
                            let is_enum = false;
                            let is_conf = !vm.non_configurable_props.contains(&(target, prop_name.clone()));
                            vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                            vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                            vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                            return Ok(desc);
                        }
                    }
                    _ => {}
                }
                if vm.shapes.has_property(obj.shape_id, prop_name) {
                    let val = vm.get_property(target, prop_name);
                    if vm.module_namespace_objects.contains(&target) && val == JsValue::uninitialized() {
                        return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", prop_name)));
                    }
                    let desc = vm.alloc_object();
                    vm.set_property(desc, "value", val);
                    let is_writ = if vm.module_namespace_objects.contains(&target) {
                        true
                    } else {
                        !vm.non_writable_props.contains(&(target, prop_name.clone()))
                    };
                    let is_enum = if vm.module_namespace_objects.contains(&target) {
                        true
                    } else {
                        !vm.non_enumerable_props.contains(&(target, prop_name.clone()))
                    };
                    let is_conf = if vm.module_namespace_objects.contains(&target) {
                        false
                    } else {
                        !vm.non_configurable_props.contains(&(target, prop_name.clone()))
                    };
                    vm.set_property(desc, "writable", JsValue::from_boolean(is_writ));
                    vm.set_property(desc, "enumerable", JsValue::from_boolean(is_enum));
                    vm.set_property(desc, "configurable", JsValue::from_boolean(is_conf));
                    return Ok(desc);
                }
                if let Some((tgt_mod, orig_name)) = vm.module_reexports.get(&(target, prop_name.clone())).cloned() {
                    let val = vm.get_property(tgt_mod, &orig_name);
                    if val == JsValue::uninitialized() {
                        return Err(VmError::ReferenceError(format!("Cannot access uninitialized export '{}'", prop_name)));
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
    let get_own_prop_desc_fn = vm.alloc_native_function("getOwnPropertyDescriptor", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Object.getOwnPropertyDescriptor called on non-object".into())),
        };
        let target_obj = if target.is_js_object() {
            target
        } else {
            vm.to_object_value(target)?
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        get_own_property_descriptor_internal(vm, target_obj, &prop_key)
    });

    let get_own_prop_descriptors_fn = vm.alloc_native_function("getOwnPropertyDescriptors", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if !t.is_nullish() => *t,
            _ => return Err(VmError::TypeError("Cannot convert undefined or null to object".into())),
        };
        let target_obj = if target.is_js_object() {
            target
        } else {
            vm.to_object_value(target)?
        };
        let keys = get_own_property_keys(vm, target_obj)?;
        let descriptors = vm.alloc_object();
        for key in keys {
            let desc = get_own_property_descriptor_internal(vm, target_obj, &key)?;
            if !desc.is_undefined() {
                match key {
                    PropertyKeyVal::String(s) => vm.set_property(descriptors, &s, desc),
                    PropertyKeyVal::Symbol(sym) => vm.set_symbol_property(descriptors, sym, desc),
                }
            }
        }
        Ok(descriptors)
    });

    (get_own_prop_desc_fn, get_own_prop_descriptors_fn)
}
