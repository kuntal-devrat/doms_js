//! The `Set` global and `Set.prototype`.

use super::*;

pub(super) fn register_set_builtins(vm: &mut Vm) {
    // ── Set ──
    let set_proto = vm.alloc_object();

    let set_add = vm.alloc_native_function("add", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &mut obj.kind {
                if !elements.iter().any(|el| crate::object::js_values_equal(*el, val)) {
                    elements.push(val);
                }
                return Ok(this);
            }
        }
        Err(VmError::TypeError("Method Set.prototype.add called on incompatible receiver".into()))
    });

    let set_has = vm.alloc_native_function("has", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                return Ok(JsValue::from_boolean(elements.iter().any(|el| crate::object::js_values_equal(*el, val))));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.has called on incompatible receiver".into()))
    });

    let set_delete = vm.alloc_native_function("delete", 1, |_vm, this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &mut obj.kind {
                if let Some(idx) = elements.iter().position(|el| crate::object::js_values_equal(*el, val)) {
                    elements.remove(idx);
                    return Ok(JsValue::from_boolean(true));
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
                elements.clear();
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
        let elements = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                elements.clone()
            } else {
                return Err(VmError::TypeError("Method Set.prototype.forEach called on incompatible receiver".into()));
            }
        } else {
            return Err(VmError::TypeError("Method Set.prototype.forEach called on incompatible receiver".into()));
        };
        for el in elements {
            let _ = vm.call(cb, this_arg, &[el, el, this])?;
        }
        Ok(JsValue::undefined())
    });

    let set_values = vm.alloc_native_function("values", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                return Ok(vm.alloc_iterator(elements.clone()));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.values called on incompatible receiver".into()))
    });

    let set_entries = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                let items: Vec<JsValue> = elements.iter().map(|el| vm.alloc_array(vec![*el, *el])).collect();
                return Ok(vm.alloc_iterator(items));
            }
        }
        Err(VmError::TypeError("Method Set.prototype.entries called on incompatible receiver".into()))
    });

    let set_size_getter = vm.alloc_native_function("get size", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Set(elements) = &obj.kind {
                return Ok(JsValue::from_int32(elements.len() as i32));
            }
        }
        Err(VmError::TypeError("Method get Set.prototype.size called on incompatible receiver".into()))
    });

    vm.set_property(set_proto, "add", set_add);
    vm.non_enumerable_props.insert((set_proto, "add".to_string()));
    vm.set_property(set_proto, "has", set_has);
    vm.non_enumerable_props.insert((set_proto, "has".to_string()));
    vm.set_property(set_proto, "delete", set_delete);
    vm.non_enumerable_props.insert((set_proto, "delete".to_string()));
    vm.set_property(set_proto, "clear", set_clear);
    vm.non_enumerable_props.insert((set_proto, "clear".to_string()));
    vm.set_property(set_proto, "forEach", set_for_each);
    vm.non_enumerable_props.insert((set_proto, "forEach".to_string()));
    vm.set_property(set_proto, "values", set_values);
    vm.non_enumerable_props.insert((set_proto, "values".to_string()));
    vm.set_property(set_proto, "keys", set_values);
    vm.non_enumerable_props.insert((set_proto, "keys".to_string()));
    vm.set_property(set_proto, "entries", set_entries);
    vm.non_enumerable_props.insert((set_proto, "entries".to_string()));
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

        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                let adder = vm.get_property(set_val, "add");
                if !vm.is_callable(adder) {
                    return Err(VmError::TypeError("Set.prototype.add is not callable".into()));
                }
                let items = iterate_to_vec(vm, *arg)?;
                for el in items {
                    vm.call(adder, set_val, &[el])?;
                }
            }
        }
        Ok(set_val)
    });
    vm.set_property(set_ctor, "prototype", set_proto);
    vm.non_enumerable_props.insert((set_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((set_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((set_ctor, "prototype".to_string()));
    vm.set_property(set_proto, "constructor", set_ctor);
    vm.non_enumerable_props.insert((set_proto, "constructor".to_string()));
    add_species_getter(vm, set_ctor);
    vm.globals.insert("Set".to_string(), set_ctor);

}
