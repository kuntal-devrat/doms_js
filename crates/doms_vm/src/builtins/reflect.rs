//! The `Reflect` global object.

use super::*;

pub(super) fn register_reflect(vm: &mut Vm) {
    let reflect_obj = vm.alloc_object();

    let construct_fn = vm.alloc_native_function("construct", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let args_list = if let Some(arg_arr) = args.get(1) {
            if let Some(ptr) = arg_arr.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    elements.clone()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let new_target = args.get(2).copied().unwrap_or(target);
        if !vm.is_constructor(target) || !vm.is_constructor(new_target) {
            return Err(VmError::TypeError("Target is not a constructor".into()));
        }
        let this_obj = vm.alloc_object();
        let proto = vm.get_property(new_target, "prototype");
        if proto.is_object() {
            let this_ptr = this_obj.as_object_ptr().unwrap();
            let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
            jsobj.prototype = Some(proto);
        }
        let prev_constructing = vm.is_constructing;
        vm.is_constructing = true;
        let res = vm.call(target, this_obj, &args_list);
        vm.is_constructing = prev_constructing;
        let res = res?;
        if res.is_object() {
            if proto.is_object() {
                if let Some(r_ptr) = res.as_object_ptr() {
                    let r_obj = unsafe { &mut *(r_ptr as *mut JsObject) };
                    r_obj.prototype = Some(proto);
                }
            }
            Ok(res)
        } else {
            Ok(this_obj)
        }
    });
    vm.set_property(reflect_obj, "construct", construct_fn);

    let apply_fn = vm.alloc_native_function("apply", 3, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let args_list = if let Some(arg_arr) = args.get(2) {
            if let Some(ptr) = arg_arr.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    elements.clone()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        vm.call(target, this_arg, &args_list)
    });
    vm.set_property(reflect_obj, "apply", apply_fn);

    let get_fn = vm.alloc_native_function("get", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        let key = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        Ok(vm.get_property(target, &key))
    });
    vm.set_property(reflect_obj, "get", get_fn);

    let set_fn = vm.alloc_native_function("set", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.set called on non-object".into())),
        };
        if vm.module_namespace_objects.contains(&target) {
            return Ok(JsValue::from_boolean(false));
        }
        let key = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let val = args.get(2).copied().unwrap_or(JsValue::undefined());
        if vm.non_writable_props.contains(&(target, key.clone())) {
            return Ok(JsValue::from_boolean(false));
        }
        vm.set_property(target, &key, val);
        Ok(JsValue::from_boolean(true))
    });
    vm.set_property(reflect_obj, "set", set_fn);

    let has_fn = vm.alloc_native_function("has", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.has called on non-object".into())),
        };
        let key_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let has = if key_arg.is_symbol() {
            let prop_key = PropertyKeyVal::Symbol(key_arg);
            let mut curr = Some(target);
            let mut found = false;
            while let Some(c_val) = curr {
                if vm.has_own_property_key(c_val, &prop_key) {
                    found = true;
                    break;
                }
                if let Some(c_ptr) = c_val.as_object_ptr() {
                    let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                    curr = c_obj.prototype;
                } else {
                    break;
                }
            }
            found
        } else {
            let key = vm.to_string_value(key_arg);
            vm.has_property(target, &key)?
        };
        Ok(JsValue::from_boolean(has))
    });
    vm.set_property(reflect_obj, "has", has_fn);

    let define_prop_fn = vm.alloc_native_function("defineProperty", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.defineProperty called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let desc = match args.get(2) {
            Some(d) if d.is_object() => *d,
            _ => return Err(VmError::TypeError("Property description must be an object".into())),
        };
        if vm.module_namespace_objects.contains(&target) {
            let success = define_property_on_module_namespace(vm, target, &prop_key, desc)?;
            return Ok(JsValue::from_boolean(success));
        }
        let obj_ctor = vm.globals.get("Object").copied().unwrap_or(JsValue::undefined());
        let def_fn = vm.get_property(obj_ctor, "defineProperty");
        let key_val = match &prop_key {
            PropertyKeyVal::Symbol(s) => *s,
            PropertyKeyVal::String(s) => vm.alloc_string(s.clone()),
        };
        let res = vm.call(def_fn, obj_ctor, &[target, key_val, desc]);
        Ok(JsValue::from_boolean(res.is_ok()))
    });
    vm.set_property(reflect_obj, "defineProperty", define_prop_fn);

    let delete_prop_fn = vm.alloc_native_function("deleteProperty", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.deleteProperty called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        if vm.module_namespace_objects.contains(&target) {
            match prop_key {
                PropertyKeyVal::Symbol(sym) => {
                    if vm.symbol_to_string_tag == Some(sym) {
                        return Ok(JsValue::from_boolean(false));
                    }
                    return Ok(JsValue::from_boolean(true));
                }
                PropertyKeyVal::String(name) => {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if vm.shapes.has_property(obj.shape_id, &name) {
                            return Ok(JsValue::from_boolean(false));
                        }
                    }
                    return Ok(JsValue::from_boolean(true));
                }
            }
        }
        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let Some(syms) = &mut obj.symbol_properties {
                        syms.remove(&sym);
                    }
                }
                Ok(JsValue::from_boolean(true))
            }
            PropertyKeyVal::String(name) => {
                if vm.non_configurable_props.contains(&(target, name.clone())) {
                    return Ok(JsValue::from_boolean(false));
                }
                vm.deleted_props.insert((target, name));
                Ok(JsValue::from_boolean(true))
            }
        }
    });
    vm.set_property(reflect_obj, "deleteProperty", delete_prop_fn);

    let get_proto_fn = vm.alloc_native_function("getPrototypeOf", 1, |_vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.getPrototypeOf called on non-object".into())),
        };
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            Ok(obj.prototype.unwrap_or(JsValue::null()))
        } else {
            Ok(JsValue::null())
        }
    });
    vm.set_property(reflect_obj, "getPrototypeOf", get_proto_fn);

    let set_proto_fn = vm.alloc_native_function("setPrototypeOf", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.setPrototypeOf called on non-object".into())),
        };
        let proto = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !proto.is_object() && !proto.is_null() {
            return Err(VmError::TypeError("Object prototype may only be an Object or null".into()));
        }
        if vm.module_namespace_objects.contains(&target) {
            return Ok(JsValue::from_boolean(proto.is_null()));
        }
        if vm.non_extensible_objects.contains(&target) {
            return Ok(JsValue::from_boolean(false));
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
            Ok(JsValue::from_boolean(true))
        } else {
            Ok(JsValue::from_boolean(false))
        }
    });
    vm.set_property(reflect_obj, "setPrototypeOf", set_proto_fn);

    let is_ext_fn = vm.alloc_native_function("isExtensible", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.isExtensible called on non-object".into())),
        };
        Ok(JsValue::from_boolean(!vm.non_extensible_objects.contains(&target)))
    });
    vm.set_property(reflect_obj, "isExtensible", is_ext_fn);

    let prev_ext_fn = vm.alloc_native_function("preventExtensions", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.preventExtensions called on non-object".into())),
        };
        vm.non_extensible_objects.insert(target);
        Ok(JsValue::from_boolean(true))
    });
    vm.set_property(reflect_obj, "preventExtensions", prev_ext_fn);

    let get_own_desc_fn = vm.alloc_native_function("getOwnPropertyDescriptor", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.getOwnPropertyDescriptor called on non-object".into())),
        };
        let prop = args.get(1).copied().unwrap_or(JsValue::undefined());
        let obj_ctor = vm.globals.get("Object").copied().unwrap_or(JsValue::undefined());
        let desc_fn = vm.get_property(obj_ctor, "getOwnPropertyDescriptor");
        vm.call(desc_fn, obj_ctor, &[target, prop])
    });
    vm.set_property(reflect_obj, "getOwnPropertyDescriptor", get_own_desc_fn);

    let own_keys_fn = vm.alloc_native_function("ownKeys", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.ownKeys called on non-object".into())),
        };
        let keys = crate::builtins::object::names::get_own_property_keys(vm, target)?;
        let mut all = Vec::with_capacity(keys.len());
        for key in keys {
            match key {
                PropertyKeyVal::String(s) => all.push(vm.alloc_string(s)),
                PropertyKeyVal::Symbol(sym) => all.push(sym),
            }
        }
        let out = vm.alloc_array(all);
        Ok(out)
    });
    vm.set_property(reflect_obj, "ownKeys", own_keys_fn);

    vm.globals.insert("Reflect".to_string(), reflect_obj);
}
