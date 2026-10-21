//! `Object.defineProperty`.

use super::descriptor::get_own_property_descriptor_internal;
use super::*;

pub(super) fn register_define_property(vm: &mut Vm) -> JsValue {
    vm.alloc_native_function("defineProperty", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => {
                return Err(VmError::TypeError(
                    "Object.defineProperty called on non-object".into(),
                ));
            }
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let desc = match args.get(2) {
            Some(d) if d.is_js_object() => *d,
            _ => return Err(VmError::TypeError("Property description must be an object".into())),
        };

        define_property_internal(vm, target, prop_key, desc)
    })
}

pub fn define_property_internal(
    vm: &mut Vm,
    target: JsValue,
    prop_key: PropertyKeyVal,
    desc: JsValue,
) -> Result<JsValue, VmError> {
    // 1. ToPropertyDescriptor(desc) per ECMAScript 6.2.6.5
    let has_enum = vm.has_property(desc, "enumerable")?;
    let enum_val =
        if has_enum { vm.get_property_result(desc, "enumerable")?.is_truthy() } else { false };

    let has_conf = vm.has_property(desc, "configurable")?;
    let conf_val =
        if has_conf { vm.get_property_result(desc, "configurable")?.is_truthy() } else { false };

    let has_val = vm.has_property(desc, "value")?;
    let val_val =
        if has_val { vm.get_property_result(desc, "value")? } else { JsValue::undefined() };

    let has_writ = vm.has_property(desc, "writable")?;
    let writ_val =
        if has_writ { vm.get_property_result(desc, "writable")?.is_truthy() } else { false };

    let has_get = vm.has_property(desc, "get")?;
    let get_val = if has_get { vm.get_property_result(desc, "get")? } else { JsValue::undefined() };

    let has_set = vm.has_property(desc, "set")?;
    let set_val = if has_set { vm.get_property_result(desc, "set")? } else { JsValue::undefined() };

    if (has_get || has_set) && (has_val || has_writ) {
        return Err(VmError::TypeError("Invalid property descriptor. Cannot both specify accessors and a value or writable attribute".into()));
    }
    if has_get && !get_val.is_undefined() && !vm.is_callable(get_val) {
        return Err(VmError::TypeError("Getter must be a function or undefined".into()));
    }
    if has_set && !set_val.is_undefined() && !vm.is_callable(set_val) {
        return Err(VmError::TypeError("Setter must be a function or undefined".into()));
    }

    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if matches!(obj.kind, ObjectKind::Proxy { .. }) {
            let ok = vm.proxy_define_own_property(target, &prop_key, desc)?;
            if !ok {
                return Err(VmError::TypeError("Proxy defineProperty trap returned false".into()));
            }
            return Ok(target);
        }
    }

    if vm.module_namespace_objects.contains(&target) {
        let success = define_property_on_module_namespace(vm, target, &prop_key, desc)?;
        if success {
            return Ok(target);
        } else {
            return Err(VmError::TypeError(
                "Cannot redefine property on module namespace object".into(),
            ));
        }
    }

    let key_name = match &prop_key {
        PropertyKeyVal::String(s) => s.clone(),
        PropertyKeyVal::Symbol(sym) => format!("[Symbol:{:x}]", sym.raw_bits()),
    };

    // Array exotic behavior for "length" and array indices
    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        let is_length_writable = obj.is_array_length_writable();
        let is_array = matches!(obj.kind, ObjectKind::Array(_));
        if is_array {
            if let PropertyKeyVal::String(s) = &prop_key {
                if s == "length" {
                    let mut validated_len = None;
                    if has_val {
                        let val = vm.get_property(desc, "value");
                        let n1 = vm.to_number_value(val)?;
                        let u32_len = if n1.is_nan() || n1.is_infinite() || n1 == 0.0 {
                            0u32
                        } else {
                            n1.rem_euclid(4294967296.0) as u32
                        };
                        let number_len = vm.to_number_value(val)?;
                        if (u32_len as f64) != number_len
                            || number_len.is_nan()
                            || number_len.is_sign_negative()
                        {
                            return Err(VmError::RangeError("Invalid array length".into()));
                        }
                        validated_len = Some((u32_len, number_len));
                    }
                    if has_get || has_set {
                        return Err(VmError::TypeError(
                            "Cannot make array length an accessor".into(),
                        ));
                    }
                    if has_conf && vm.get_property(desc, "configurable").is_truthy() {
                        return Err(VmError::TypeError(
                            "Cannot make array length configurable".into(),
                        ));
                    }
                    if has_enum && vm.get_property(desc, "enumerable").is_truthy() {
                        return Err(VmError::TypeError(
                            "Cannot make array length enumerable".into(),
                        ));
                    }

                    let is_length_writable = if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        obj.is_array_length_writable()
                    } else {
                        true
                    };

                    if let Some((u32_len, num)) = validated_len {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        let old_len = if let ObjectKind::Array(elements) = &obj.kind {
                            elements.len()
                        } else {
                            0
                        };
                        let new_len = u32_len as usize;

                        if new_len >= old_len {
                            if !is_length_writable && new_len != old_len {
                                return Err(VmError::TypeError(
                                    "Cannot change length of array with non-writable length".into(),
                                ));
                            }
                            if let ObjectKind::Array(elements) = &mut obj.kind {
                                if new_len < 100_000 {
                                    elements.resize(new_len, JsValue::hole());
                                } else {
                                    vm.set_own_property(
                                        target,
                                        "length",
                                        JsValue::from_double(num),
                                    );
                                }
                            }
                        } else {
                            if !is_length_writable {
                                return Err(VmError::TypeError(
                                    "Cannot change length of array with non-writable length".into(),
                                ));
                            }
                            let mut non_conf_conflict: Option<usize> = None;
                            for i in (new_len..old_len).rev() {
                                if !vm.is_own_property_configurable(target, &i.to_string()) {
                                    non_conf_conflict = Some(i);
                                    break;
                                }
                            }
                            let obj = unsafe { &mut *(ptr as *mut JsObject) };
                            if let ObjectKind::Array(elements) = &mut obj.kind {
                                if let Some(i) = non_conf_conflict {
                                    elements.truncate(i + 1);
                                } else {
                                    elements.truncate(new_len);
                                }
                            }
                            if non_conf_conflict.is_some() {
                                if has_writ && !vm.get_property(desc, "writable").is_truthy() {
                                    obj.set_array_length_non_writable();
                                }
                                return Err(VmError::TypeError(
                                    "Cannot delete non-configurable array element".into(),
                                ));
                            }
                        }
                    }
                    if has_writ {
                        let w = vm.get_property(desc, "writable").is_truthy();
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if !w {
                            obj.set_array_length_non_writable();
                        } else if !is_length_writable {
                            return Err(VmError::TypeError(
                                "Cannot make non-writable length writable".into(),
                            ));
                        }
                    }
                    return Ok(target);
                } else if let Ok(idx) = s.parse::<u32>() {
                    if idx.to_string() == *s {
                        let idx = idx as usize;
                        let old_len = if let ObjectKind::Array(elements) = &obj.kind {
                            elements.len()
                        } else {
                            0
                        };
                        if idx >= old_len && !is_length_writable {
                            return Err(VmError::TypeError(
                                "Cannot add property to array with non-writable length".into(),
                            ));
                        }
                    }
                }
            }
        }
    }

    let current = get_own_property_descriptor_internal(vm, target, &prop_key)?;
    let current_exists = !current.is_undefined();

    if !current_exists {
        if !vm.is_extensible_object(target) {
            return Err(VmError::TypeError(format!(
                "Cannot define property {}, object is not extensible",
                key_name
            )));
        }
    } else {
        let curr_conf = vm.get_property(current, "configurable").is_truthy();
        let curr_enum = vm.get_property(current, "enumerable").is_truthy();
        let curr_is_accessor = vm.has_property(current, "get").unwrap_or(false)
            || vm.has_property(current, "set").unwrap_or(false);

        if !curr_conf {
            if has_conf && vm.get_property(desc, "configurable").is_truthy() {
                return Err(VmError::TypeError(
                    "Cannot redefine property: configurable is false".into(),
                ));
            }
            if has_enum && vm.get_property(desc, "enumerable").is_truthy() != curr_enum {
                return Err(VmError::TypeError(
                    "Cannot redefine enumerable on non-configurable property".into(),
                ));
            }
            let is_accessor_desc = has_get || has_set;
            let is_data_desc = has_val || has_writ;
            if (is_accessor_desc && !curr_is_accessor) || (is_data_desc && curr_is_accessor) {
                return Err(VmError::TypeError(
                    "Cannot switch between data and accessor property on non-configurable property"
                        .into(),
                ));
            }
            if curr_is_accessor {
                if has_get {
                    let curr_get = vm.get_property(current, "get");
                    if !vm.same_value(get_val, curr_get) {
                        return Err(VmError::TypeError(
                            "Cannot change getter on non-configurable property".into(),
                        ));
                    }
                }
                if has_set {
                    let curr_set = vm.get_property(current, "set");
                    if !vm.same_value(set_val, curr_set) {
                        return Err(VmError::TypeError(
                            "Cannot change setter on non-configurable property".into(),
                        ));
                    }
                }
            } else {
                let curr_writ = vm.get_property(current, "writable").is_truthy();
                if !curr_writ {
                    if has_writ && vm.get_property(desc, "writable").is_truthy() {
                        return Err(VmError::TypeError("Cannot make non-writable property writable on non-configurable property".into()));
                    }
                    if has_val {
                        let new_val = vm.get_property(desc, "value");
                        let curr_val = vm.get_property(current, "value");
                        if !vm.same_value(new_val, curr_val) {
                            return Err(VmError::TypeError(
                                "Cannot change value of non-writable non-configurable property"
                                    .into(),
                            ));
                        }
                    }
                }
            }
        }
    }

    let is_accessor = has_get
        || has_set
        || (current_exists
            && (vm.has_property(current, "get").unwrap_or(false)
                || vm.has_property(current, "set").unwrap_or(false))
            && !has_val
            && !has_writ);

    match &prop_key {
        PropertyKeyVal::Symbol(sym) => {
            let sym = *sym;
            if is_accessor {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let Some(syms) = &mut obj.symbol_properties {
                        syms.remove(&sym);
                    }
                    let accs = obj
                        .symbol_accessors
                        .get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    let entry = accs.entry(sym).or_insert_with(Accessor::default);
                    if has_get {
                        entry.getter = if get_val.is_undefined() { None } else { Some(get_val) };
                    }
                    if has_set {
                        entry.setter = if set_val.is_undefined() { None } else { Some(set_val) };
                    }
                    if has_enum {
                        entry.enumerable = enum_val;
                    } else if !current_exists {
                        entry.enumerable = false;
                    }
                    if has_conf {
                        entry.configurable = conf_val;
                    } else if !current_exists {
                        entry.configurable = false;
                    }
                    let order = obj.symbol_order.get_or_insert_with(Vec::new);
                    if !order.contains(&sym) {
                        order.push(sym);
                    }
                }
            } else {
                if has_val || !current_exists {
                    let val =
                        if has_val { vm.get_property(desc, "value") } else { JsValue::undefined() };
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if let Some(accs) = &mut obj.symbol_accessors {
                            accs.remove(&sym);
                        }
                        obj.insert_symbol_property(sym, val);
                        let curr_attrs = obj
                            .symbol_attributes
                            .as_ref()
                            .and_then(|m| m.get(&sym))
                            .copied()
                            .unwrap_or(PropertyAttributes::DEFAULT);
                        let attrs = PropertyAttributes {
                            writable: if has_writ {
                                writ_val
                            } else if !current_exists {
                                false
                            } else {
                                curr_attrs.writable
                            },
                            enumerable: if has_enum {
                                enum_val
                            } else if !current_exists {
                                false
                            } else {
                                curr_attrs.enumerable
                            },
                            configurable: if has_conf {
                                conf_val
                            } else if !current_exists {
                                false
                            } else {
                                curr_attrs.configurable
                            },
                        };
                        obj.set_symbol_attributes(sym, attrs);
                    }
                } else if has_writ || has_enum || has_conf {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        let curr_attrs = obj
                            .symbol_attributes
                            .as_ref()
                            .and_then(|m| m.get(&sym))
                            .copied()
                            .unwrap_or(PropertyAttributes::DEFAULT);
                        let attrs = PropertyAttributes {
                            writable: if has_writ { writ_val } else { curr_attrs.writable },
                            enumerable: if has_enum { enum_val } else { curr_attrs.enumerable },
                            configurable: if has_conf { conf_val } else { curr_attrs.configurable },
                        };
                        obj.set_symbol_attributes(sym, attrs);
                    }
                }
            }
        }
        PropertyKeyVal::String(prop_name) => {
            // Element-level attributes on arrays (index descriptors, frozen
            // arrays, sparse indices) must disable the dense fast paths.
            let is_array = target.as_object_ptr().is_some_and(|p| {
                matches!(unsafe { &*(p as *const JsObject) }.kind, ObjectKind::Array(_))
            });
            if is_array && Vm::canonical_array_index(prop_name).is_some() {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    obj.mark_element_attributes();
                }
            }
            if is_accessor {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    let new_shape_id = vm.shapes.remove_property(obj.shape_id, prop_name);
                    obj.shape_id = new_shape_id;
                    let accs = obj
                        .accessors
                        .get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    let entry = accs.entry(prop_name.clone()).or_insert_with(Accessor::default);
                    if has_get {
                        entry.getter = if get_val.is_undefined() { None } else { Some(get_val) };
                    }
                    if has_set {
                        entry.setter = if set_val.is_undefined() { None } else { Some(set_val) };
                    }
                    if has_enum {
                        entry.enumerable = enum_val;
                    } else if !current_exists {
                        entry.enumerable = false;
                    }
                    if has_conf {
                        entry.configurable = conf_val;
                    } else if !current_exists {
                        entry.configurable = false;
                    }
                    let order = obj.accessor_order.get_or_insert_with(Vec::new);
                    if !order.contains(prop_name) {
                        order.push(prop_name.clone());
                    }
                }
            } else {
                let current_info = if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *const JsObject) };
                    vm.shapes.get_property_info(obj.shape_id, prop_name)
                } else {
                    None
                };
                let curr_writ = if let Some(ref info) = current_info {
                    info.attributes.writable
                } else if current_exists {
                    vm.get_property(current, "writable").is_truthy()
                } else {
                    false
                };
                let curr_enum = if let Some(ref info) = current_info {
                    info.attributes.enumerable
                } else if current_exists {
                    vm.get_property(current, "enumerable").is_truthy()
                } else {
                    false
                };
                let curr_conf = if let Some(ref info) = current_info {
                    info.attributes.configurable
                } else if current_exists {
                    vm.get_property(current, "configurable").is_truthy()
                } else {
                    false
                };
                let attrs = crate::shape::PropertyAttributes {
                    writable: if has_writ { writ_val } else { curr_writ },
                    enumerable: if has_enum { enum_val } else { curr_enum },
                    configurable: if has_conf { conf_val } else { curr_conf },
                };
                let val = if has_val {
                    val_val
                } else if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *const JsObject) };
                    if let ObjectKind::Array(elements) = &obj.kind {
                        if let Ok(idx) = prop_name.parse::<usize>() {
                            if idx < elements.len() {
                                elements[idx]
                            } else if let Some(ref info) = current_info {
                                obj.get_slot(info.offset).unwrap_or(JsValue::undefined())
                            } else {
                                JsValue::undefined()
                            }
                        } else if let Some(ref info) = current_info {
                            obj.get_slot(info.offset).unwrap_or(JsValue::undefined())
                        } else {
                            JsValue::undefined()
                        }
                    } else if let Some(ref info) = current_info {
                        obj.get_slot(info.offset).unwrap_or(JsValue::undefined())
                    } else {
                        JsValue::undefined()
                    }
                } else {
                    JsValue::undefined()
                };

                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let Some(accs) = &mut obj.accessors {
                        accs.remove(prop_name);
                    }
                    if let Some(order) = &mut obj.accessor_order {
                        order.retain(|k| k != prop_name);
                    }
                    if let ObjectKind::Array(elements) = &mut obj.kind {
                        if let Ok(idx) = prop_name.parse::<u32>() {
                            if idx.to_string() == *prop_name {
                                let idx = idx as usize;
                                if idx >= elements.len() {
                                    if idx < 100_000 {
                                        elements.resize(idx + 1, JsValue::hole());
                                    } else {
                                        vm.set_own_property(
                                            target,
                                            "length",
                                            JsValue::from_double((idx as f64) + 1.0),
                                        );
                                    }
                                }
                                if idx < elements.len() {
                                    elements[idx] = val;
                                }
                            }
                        }
                    }
                    if let Some(offset) = vm.shapes.lookup_offset(obj.shape_id, prop_name) {
                        let new_shape_id =
                            vm.shapes.transition_redefine(obj.shape_id, prop_name, attrs);
                        obj.shape_id = new_shape_id;
                        obj.set_slot(offset, val);
                    } else {
                        let (new_shape_id, offset) =
                            vm.shapes.transition_with_attributes(obj.shape_id, prop_name, attrs);
                        obj.shape_id = new_shape_id;
                        obj.set_slot(offset, val);
                        obj.add_prop_order(prop_name.clone());
                    }
                }
                if target == vm.global_object {
                    vm.globals.insert(prop_name.clone(), val);
                }
                vm.sync_mapped_arguments_write(target, prop_name, val);
            }
        }
    }

    if !is_accessor {
        if (has_writ && !writ_val) || (!has_writ && !current_exists) {
            if let PropertyKeyVal::String(prop_name) = &prop_key {
                vm.sync_mapped_arguments_delete(target, prop_name);
            }
        }
    }

    if let Some(ptr) = target.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        if let ObjectKind::Array(elements) = &mut obj.kind {
            if let PropertyKeyVal::String(s) = &prop_key {
                if let Ok(idx) = s.parse::<u32>() {
                    if idx.to_string() == *s {
                        let idx = idx as usize;
                        if idx >= elements.len() {
                            if idx < 100_000 {
                                elements.resize(idx + 1, JsValue::hole());
                            } else {
                                vm.set_own_property(
                                    target,
                                    "length",
                                    JsValue::from_double((idx as f64) + 1.0),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(target)
}
