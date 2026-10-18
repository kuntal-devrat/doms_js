//! The `Set` global and `Set.prototype`.

use super::*;
use crate::builtins::promise::iterator::{iterator_close, IteratorRecord};

struct SetRecord {
    set: JsValue,
    size: f64,
    has: JsValue,
    keys: JsValue,
}

fn get_set_record(vm: &mut Vm, other: JsValue) -> Result<SetRecord, VmError> {
    if !other.is_object() {
        return Err(VmError::TypeError("GetSetRecord called on non-object".into()));
    }
    let raw_size = vm.get_property_result(other, "size")?;
    let num_size = vm.to_number_value(raw_size)?;
    if num_size.is_nan() {
        return Err(VmError::TypeError("Set size must be a number".into()));
    }
    let int_size = if num_size.is_infinite() {
        num_size
    } else {
        num_size.trunc()
    };
    if int_size < 0.0 {
        return Err(VmError::RangeError("Set size must be non-negative".into()));
    }
    let has = vm.get_property_result(other, "has")?;
    if !vm.is_callable(has) {
        return Err(VmError::TypeError("Set has must be a function".into()));
    }
    let keys = vm.get_property_result(other, "keys")?;
    if !vm.is_callable(keys) {
        return Err(VmError::TypeError("Set keys must be a function".into()));
    }
    Ok(SetRecord {
        set: other,
        size: int_size,
        has,
        keys,
    })
}

fn get_keys_iterator(vm: &mut Vm, rec: &SetRecord) -> Result<IteratorRecord, VmError> {
    let keys_iter = vm.call(rec.keys, rec.set, &[])?;
    if !keys_iter.is_object() {
        return Err(VmError::TypeError("Set keys must return an object".into()));
    }
    let next_method = vm.get_property_result(keys_iter, "next")?;
    if !vm.is_callable(next_method) {
        return Err(VmError::TypeError("Iterator next must be callable".into()));
    }
    Ok(IteratorRecord {
        iterator: keys_iter,
        next_method,
        done: false,
    })
}

fn step_iterator(vm: &mut Vm, rec: &IteratorRecord) -> Result<Option<JsValue>, VmError> {
    let step = vm.call(rec.next_method, rec.iterator, &[])?;
    if !step.is_object() {
        return Err(VmError::TypeError("Iterator result must be an object".into()));
    }
    let done = vm.get_property_result(step, "done")?.is_truthy();
    if done {
        return Ok(None);
    }
    let val = vm.get_property_result(step, "value")?;
    Ok(Some(val))
}

fn get_set_elements(this: JsValue) -> Result<Vec<JsValue>, VmError> {
    if let Some(ptr) = this.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Set(elements) = &obj.kind {
            return Ok(elements.iter().filter_map(|e| *e).collect());
        }
    }
    Err(VmError::TypeError("Method called on incompatible receiver".into()))
}

fn normalize_set_element(el: JsValue) -> JsValue {
    if el.to_number() == Some(0.0) {
        JsValue::from_double(0.0)
    } else {
        el
    }
}

fn set_add_element(set_val: JsValue, el: JsValue) {
    let el = normalize_set_element(el);
    if let Some(ptr) = set_val.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        if let ObjectKind::Set(elements) = &mut obj.kind {
            if !elements.iter().any(|existing| existing.as_ref().map_or(false, |e| crate::object::js_values_equal(*e, el))) {
                elements.push(Some(el));
            }
        }
    }
}

fn set_delete_element(set_val: JsValue, el: JsValue) -> bool {
    let el = normalize_set_element(el);
    if let Some(ptr) = set_val.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        if let ObjectKind::Set(elements) = &mut obj.kind {
            for existing in elements.iter_mut() {
                if let Some(e) = existing {
                    if crate::object::js_values_equal(*e, el) {
                        *existing = None;
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn set_has_element(set_val: JsValue, el: JsValue) -> bool {
    let el = normalize_set_element(el);
    if let Some(ptr) = set_val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Set(elements) = &obj.kind {
            return elements.iter().any(|existing| existing.as_ref().map_or(false, |e| crate::object::js_values_equal(*e, el)));
        }
    }
    false
}

fn alloc_set_with_elements(vm: &mut Vm, elements: &[JsValue]) -> JsValue {
    let set_val = vm.alloc_set();
    if let Some(ptr) = set_val.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        let mut set_els: Vec<Option<JsValue>> = Vec::with_capacity(elements.len());
        for &el in elements {
            let el = normalize_set_element(el);
            if !set_els.iter().any(|existing| existing.as_ref().map_or(false, |e| crate::object::js_values_equal(*e, el))) {
                set_els.push(Some(el));
            }
        }
        obj.kind = ObjectKind::Set(set_els);
    }
    set_val
}

pub(super) fn register_set_builtins(vm: &mut Vm) {
    // Set prototype and constructor
    let set_proto = vm.alloc_object();

    let set_add = vm.alloc_native_function("add", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let val = normalize_set_element(val);
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &mut obj.kind {
                if !elements.iter().any(|el| el.as_ref().map_or(false, |e| crate::object::js_values_equal(*e, val))) {
                    elements.push(Some(val));
                }
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method Set.prototype.add called on incompatible receiver".into()))
    });

    let set_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let val = normalize_set_element(val);
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                return Ok(JsValue::from_boolean(elements.iter().any(|el| el.as_ref().map_or(false, |e| crate::object::js_values_equal(*e, val)))));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.has called on incompatible receiver".into()))
    });

    let set_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let val = normalize_set_element(val);
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &mut obj.kind {
                for el in elements.iter_mut() {
                    if let Some(e) = el {
                        if crate::object::js_values_equal(*e, val) {
                            *el = None;
                            return Ok(JsValue::from_boolean(true));
                        }
                    }
                }
                return Ok(JsValue::from_boolean(false));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.delete called on incompatible receiver".into()))
    });

    let set_clear = vm.alloc_native_function("clear", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &mut obj.kind {
                for el in elements.iter_mut() {
                    *el = None;
                }
                return Ok(JsValue::undefined());
            }
        }
        Err(VmError::TypeError("Method Set.prototype.clear called on incompatible receiver".into()))
    });

    let set_for_each = vm.alloc_native_function("forEach", 1, |vm, this, args| {
        let cb = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(cb) {
            return Err(VmError::TypeError("Callback is not a function".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let mut idx = 0;
        loop {
            let el = if let Some(ptr) = this.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Set(elements) = &obj.kind {
                    if idx < elements.len() {
                        elements[idx]
                    } else {
                        break;
                    }
                } else {
                    return Err(VmError::TypeError("Method Set.prototype.forEach called on incompatible receiver".into()));
                }
            } else {
                return Err(VmError::TypeError("Method Set.prototype.forEach called on incompatible receiver".into()));
            };
            idx += 1;
            if let Some(el) = el {
                let _ = vm.call(cb, this_arg, &[el, el, this])?;
            }
        }
        Ok(JsValue::undefined())
    });

    let set_values = vm.alloc_native_function("values", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(_) = &obj.kind {
                return Ok(vm.alloc_set_iterator(this, SetIterationKind::Value));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.values called on incompatible receiver".into()))
    });

    let set_entries = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(_) = &obj.kind {
                return Ok(vm.alloc_set_iterator(this, SetIterationKind::KeyValue));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.entries called on incompatible receiver".into()))
    });

    let set_size_getter = vm.alloc_native_function("get size", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                let count = elements.iter().filter(|el| el.is_some()).count();
                return Ok(JsValue::from_int32(count as i32));
            }
        }
        Err(VmError::TypeError("Method get Set.prototype.size called on incompatible receiver".into()))
    });

    // ES2024 Set methods
    let set_intersection = vm.alloc_native_function("intersection", 1, |vm, this, args| {
        let elements = get_set_elements(this)?;
        let other_val = args.first().copied().unwrap_or(JsValue::undefined());
        let other_rec = get_set_record(vm, other_val)?;

        let result_set = vm.alloc_set();
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(result_set);

        let this_size = elements.len() as f64;
        if this_size <= other_rec.size {
            for e in elements {
                let in_other = vm.call(other_rec.has, other_rec.set, &[e])?.is_truthy();
                if in_other {
                    set_add_element(result_set, e);
                }
            }
        } else {
            let iter_rec = get_keys_iterator(vm, &other_rec)?;
            loop {
                let next = match step_iterator(vm, &iter_rec) {
                    Ok(v) => v,
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(iterator_close(vm, &iter_rec, Some(e)));
                    }
                };
                let Some(next_val) = next else { break; };
                let already_in = set_has_element(result_set, next_val);
                if !already_in {
                    let in_this = set_has_element(this, next_val);
                    if in_this {
                        set_add_element(result_set, next_val);
                    }
                }
            }
        }

        vm.temp_roots.truncate(prev_temp);
        Ok(result_set)
    });

    let set_union = vm.alloc_native_function("union", 1, |vm, this, args| {
        let elements = get_set_elements(this)?;
        let other_val = args.first().copied().unwrap_or(JsValue::undefined());
        let other_rec = get_set_record(vm, other_val)?;

        let result_set = alloc_set_with_elements(vm, &elements);
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(result_set);

        let iter_rec = get_keys_iterator(vm, &other_rec)?;
        loop {
            let next = match step_iterator(vm, &iter_rec) {
                Ok(v) => v,
                Err(e) => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(iterator_close(vm, &iter_rec, Some(e)));
                }
            };
            let Some(next_val) = next else { break; };
            set_add_element(result_set, next_val);
        }

        vm.temp_roots.truncate(prev_temp);
        Ok(result_set)
    });

    let set_difference = vm.alloc_native_function("difference", 1, |vm, this, args| {
        let elements = get_set_elements(this)?;
        let other_val = args.first().copied().unwrap_or(JsValue::undefined());
        let other_rec = get_set_record(vm, other_val)?;

        let this_size = elements.len() as f64;
        let result_set = if this_size <= other_rec.size {
            let res = vm.alloc_set();
            let prev_temp = vm.temp_roots.len();
            vm.temp_roots.push(res);
            for e in elements {
                let in_other = vm.call(other_rec.has, other_rec.set, &[e])?.is_truthy();
                if !in_other {
                    set_add_element(res, e);
                }
            }
            vm.temp_roots.truncate(prev_temp);
            res
        } else {
            let res = alloc_set_with_elements(vm, &elements);
            let prev_temp = vm.temp_roots.len();
            vm.temp_roots.push(res);
            let iter_rec = get_keys_iterator(vm, &other_rec)?;
            loop {
                let next = match step_iterator(vm, &iter_rec) {
                    Ok(v) => v,
                    Err(e) => {
                        vm.temp_roots.truncate(prev_temp);
                        return Err(iterator_close(vm, &iter_rec, Some(e)));
                    }
                };
                let Some(next_val) = next else { break; };
                set_delete_element(res, next_val);
            }
            vm.temp_roots.truncate(prev_temp);
            res
        };

        Ok(result_set)
    });

    let set_symmetric_difference = vm.alloc_native_function("symmetricDifference", 1, |vm, this, args| {
        let elements = get_set_elements(this)?;
        let other_val = args.first().copied().unwrap_or(JsValue::undefined());
        let other_rec = get_set_record(vm, other_val)?;

        let result_set = alloc_set_with_elements(vm, &elements);
        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(result_set);

        let iter_rec = get_keys_iterator(vm, &other_rec)?;
        loop {
            let next = match step_iterator(vm, &iter_rec) {
                Ok(v) => v,
                Err(e) => {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(iterator_close(vm, &iter_rec, Some(e)));
                }
            };
            let Some(next_val) = next else { break; };
            let in_this = set_has_element(this, next_val);
            if in_this {
                set_delete_element(result_set, next_val);
            } else {
                set_add_element(result_set, next_val);
            }
        }

        vm.temp_roots.truncate(prev_temp);
        Ok(result_set)
    });

    let set_is_subset_of = vm.alloc_native_function("isSubsetOf", 1, |vm, this, args| {
        let elements = get_set_elements(this)?;
        let other_val = args.first().copied().unwrap_or(JsValue::undefined());
        let other_rec = get_set_record(vm, other_val)?;

        if (elements.len() as f64) > other_rec.size {
            return Ok(JsValue::from_boolean(false));
        }

        for e in elements {
            let in_other = vm.call(other_rec.has, other_rec.set, &[e])?.is_truthy();
            if !in_other {
                return Ok(JsValue::from_boolean(false));
            }
        }

        Ok(JsValue::from_boolean(true))
    });

    let set_is_superset_of = vm.alloc_native_function("isSupersetOf", 1, |vm, this, args| {
        let elements = get_set_elements(this)?;
        let other_val = args.first().copied().unwrap_or(JsValue::undefined());
        let other_rec = get_set_record(vm, other_val)?;

        if (elements.len() as f64) < other_rec.size {
            return Ok(JsValue::from_boolean(false));
        }

        let iter_rec = get_keys_iterator(vm, &other_rec)?;
        loop {
            let next = match step_iterator(vm, &iter_rec) {
                Ok(v) => v,
                Err(e) => return Err(iterator_close(vm, &iter_rec, Some(e))),
            };
            let Some(next_val) = next else { break; };
            let in_this = set_has_element(this, next_val);
            if !in_this {
                let _ = iterator_close(vm, &iter_rec, None);
                return Ok(JsValue::from_boolean(false));
            }
        }

        Ok(JsValue::from_boolean(true))
    });

    let set_is_disjoint_from = vm.alloc_native_function("isDisjointFrom", 1, |vm, this, args| {
        let elements = get_set_elements(this)?;
        let other_val = args.first().copied().unwrap_or(JsValue::undefined());
        let other_rec = get_set_record(vm, other_val)?;

        let this_size = elements.len() as f64;
        if this_size <= other_rec.size {
            for e in elements {
                let in_other = vm.call(other_rec.has, other_rec.set, &[e])?.is_truthy();
                if in_other {
                    return Ok(JsValue::from_boolean(false));
                }
            }
        } else {
            let iter_rec = get_keys_iterator(vm, &other_rec)?;
            loop {
                let next = match step_iterator(vm, &iter_rec) {
                    Ok(v) => v,
                    Err(e) => return Err(iterator_close(vm, &iter_rec, Some(e))),
                };
                let Some(next_val) = next else { break; };
                let in_this = set_has_element(this, next_val);
                if in_this {
                    let _ = iterator_close(vm, &iter_rec, None);
                    return Ok(JsValue::from_boolean(false));
                }
            }
        }

        Ok(JsValue::from_boolean(true))
    });

    let methods = [
        ("add", set_add),
        ("has", set_has),
        ("delete", set_delete),
        ("clear", set_clear),
        ("forEach", set_for_each),
        ("values", set_values),
        ("keys", set_values),
        ("entries", set_entries),
        ("intersection", set_intersection),
        ("union", set_union),
        ("difference", set_difference),
        ("symmetricDifference", set_symmetric_difference),
        ("isSubsetOf", set_is_subset_of),
        ("isSupersetOf", set_is_superset_of),
        ("isDisjointFrom", set_is_disjoint_from),
    ];

    for (name, func) in methods {
        vm.set_property(set_proto, name, func);
        vm.non_enumerable_props.insert((set_proto, name.to_string()));
    }

    if let Some(ptr) = set_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_getter("size".to_string(), set_size_getter);
    }
    vm.non_enumerable_props.insert((set_proto, "size".to_string()));

    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(set_proto, sym_iter, set_values);
    }
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Set".into());
        vm.set_symbol_property(set_proto, sym_tag, tag_str);
    }

    let set_ctor = vm.alloc_native_function("Set", 0, |vm, this, args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError("Constructor Set requires 'new'".into()));
        }
        let set_val = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Ordinary) {
                obj.kind = ObjectKind::Set(Vec::new());
                this
            } else {
                vm.alloc_set()
            }
        } else {
            vm.alloc_set()
        };

        let prev_temp = vm.temp_roots.len();
        vm.temp_roots.push(set_val);

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let cur_set = vm.temp_roots[prev_temp];
                let adder = vm.get_property(cur_set, "add");
                if !vm.is_callable(adder) {
                    vm.temp_roots.truncate(prev_temp);
                    return Err(VmError::TypeError("Set.prototype.add is not callable".into()));
                }
                vm.temp_roots.push(adder);
                let arg_val = *arg;
                let items = iterate_to_vec(vm, arg_val)?;
                let items_start = vm.temp_roots.len();
                for item in items {
                    vm.temp_roots.push(item);
                }
                let num_items = vm.temp_roots.len() - items_start;
                for i in 0..num_items {
                    let cur_item = vm.temp_roots[items_start + i];
                    let cur_set = vm.temp_roots[prev_temp];
                    let cur_adder = vm.temp_roots[prev_temp + 1];
                    vm.call(cur_adder, cur_set, &[cur_item])?;
                }
                vm.temp_roots.truncate(prev_temp + 1);
            }
        }
        let res = vm.temp_roots[prev_temp];
        vm.temp_roots.truncate(prev_temp);
        Ok(res)
    });
    vm.set_property(set_ctor, "prototype", set_proto);
    vm.non_enumerable_props.insert((set_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((set_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((set_ctor, "prototype".to_string()));
    vm.set_property(set_proto, "constructor", set_ctor);
    vm.non_enumerable_props.insert((set_proto, "constructor".to_string()));
    add_species_getter(vm, set_ctor);
    vm.globals.insert("Set".to_string(), set_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Set", set_ctor);
    }
}
