//! The `Reflect` global object.

use super::*;
use crate::builtins::helpers::define_property_on_module_namespace;
use crate::builtins::object::descriptor::get_own_property_descriptor_internal;
use crate::builtins::object::names::get_own_property_keys;
use crate::interpreter::PropertyKeyVal;

fn create_list_from_array_like(vm: &mut Vm, val: JsValue) -> Result<Vec<JsValue>, VmError> {
    if !val.is_js_object() {
        return Err(VmError::TypeError("CreateListFromArrayLike called on non-object".into()));
    }
    let len_val = vm.get_property_result(val, "length")?;
    let len = len_val.to_number().unwrap_or(0.0).max(0.0) as usize;
    let mut list = Vec::with_capacity(len.min(10000));
    for i in 0..len {
        let elem = vm.get_property_result(val, &i.to_string())?;
        list.push(elem);
    }
    Ok(list)
}

pub(super) fn register_reflect(vm: &mut Vm) {
    let reflect_obj = vm.alloc_object();

    // 28.1.1 Reflect.apply ( target, thisArgument, argumentsList )
    let apply_fn = vm.alloc_native_function("apply", 3, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(target) {
            return Err(VmError::TypeError("Reflect.apply: target is not callable".into()));
        }
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let arg_list_val = match args.get(2) {
            Some(v) if v.is_js_object() => *v,
            _ => return Err(VmError::TypeError("Reflect.apply: argumentsList is not an object".into())),
        };
        let args_list = create_list_from_array_like(vm, arg_list_val)?;
        vm.call(target, this_arg, &args_list)
    });
    vm.set_property(reflect_obj, "apply", apply_fn);

    // 28.1.2 Reflect.construct ( target, argumentsList [ , newTarget ] )
    let construct_fn = vm.alloc_native_function("construct", 2, |vm, _this, args| {
        let target = args.first().copied().unwrap_or(JsValue::undefined());
        if !vm.is_constructor(target) {
            return Err(VmError::TypeError("Reflect.construct: target is not a constructor".into()));
        }
        let new_target = args.get(2).copied().unwrap_or(target);
        if !vm.is_constructor(new_target) {
            return Err(VmError::TypeError("Reflect.construct: newTarget is not a constructor".into()));
        }
        let arg_list_val = match args.get(1) {
            Some(v) if v.is_js_object() => *v,
            _ => return Err(VmError::TypeError("Reflect.construct: argumentsList is not an object".into())),
        };
        let args_list = create_list_from_array_like(vm, arg_list_val)?;
        vm.construct_with_new_target(target, &args_list, new_target)
    });
    vm.set_property(reflect_obj, "construct", construct_fn);

    // 28.1.3 Reflect.defineProperty ( target, propertyKey, attributes )
    let define_prop_fn = vm.alloc_native_function("defineProperty", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.defineProperty called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let desc = match args.get(2) {
            Some(d) if d.is_js_object() => *d,
            _ => return Err(VmError::TypeError("Property description must be an object".into())),
        };
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                let success = vm.proxy_define_own_property(target, &prop_key, desc)?;
                return Ok(JsValue::from_boolean(success));
            }
        }
        if vm.module_namespace_objects.contains(&target) {
            let success = define_property_on_module_namespace(vm, target, &prop_key, desc)?;
            return Ok(JsValue::from_boolean(success));
        }
        let res = crate::builtins::object::define_property::define_property_internal(vm, target, prop_key, desc);
        match res {
            Ok(_) => Ok(JsValue::from_boolean(true)),
            Err(VmError::TypeError(_)) => Ok(JsValue::from_boolean(false)),
            Err(e) => Err(e),
        }
    });
    vm.set_property(reflect_obj, "defineProperty", define_prop_fn);

    // 28.1.4 Reflect.deleteProperty ( target, propertyKey )
    let delete_prop_fn = vm.alloc_native_function("deleteProperty", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.deleteProperty called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let ok = vm.delete_property_key(target, &prop_key)?;
        Ok(JsValue::from_boolean(ok))
    });
    vm.set_property(reflect_obj, "deleteProperty", delete_prop_fn);

    // 28.1.5 Reflect.get ( target, propertyKey [ , receiver ] )
    let get_fn = vm.alloc_native_function("get", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.get called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let receiver = args.get(2).copied().unwrap_or(target);
        match prop_key {
            PropertyKeyVal::String(s) => vm.get_property_with_receiver_result(target, &s, receiver),
            PropertyKeyVal::Symbol(sym) => {
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                        return vm.proxy_get(target, &prop_key, receiver);
                    }
                }
                vm.get_symbol_property_result(receiver, sym)
            }
        }
    });
    vm.set_property(reflect_obj, "get", get_fn);

    // 28.1.6 Reflect.getOwnPropertyDescriptor ( target, propertyKey )
    let get_own_desc_fn = vm.alloc_native_function("getOwnPropertyDescriptor", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.getOwnPropertyDescriptor called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        get_own_property_descriptor_internal(vm, target, &prop_key)
    });
    vm.set_property(reflect_obj, "getOwnPropertyDescriptor", get_own_desc_fn);

    // 28.1.7 Reflect.getPrototypeOf ( target )
    let get_proto_fn = vm.alloc_native_function("getPrototypeOf", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.getPrototypeOf called on non-object".into())),
        };
        vm.get_prototype_of_val(target)
    });
    vm.set_property(reflect_obj, "getPrototypeOf", get_proto_fn);

    // 28.1.8 Reflect.has ( target, propertyKey )
    let has_fn = vm.alloc_native_function("has", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.has called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let has = vm.has_property_key(target, &prop_key)?;
        Ok(JsValue::from_boolean(has))
    });
    vm.set_property(reflect_obj, "has", has_fn);

    // 28.1.9 Reflect.isExtensible ( target )
    let is_ext_fn = vm.alloc_native_function("isExtensible", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.isExtensible called on non-object".into())),
        };
        let ext = vm.is_extensible_val(target)?;
        Ok(JsValue::from_boolean(ext))
    });
    vm.set_property(reflect_obj, "isExtensible", is_ext_fn);

    // 28.1.10 Reflect.ownKeys ( target )
    let own_keys_fn = vm.alloc_native_function("ownKeys", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.ownKeys called on non-object".into())),
        };
        let keys = get_own_property_keys(vm, target)?;
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

    // 28.1.11 Reflect.preventExtensions ( target )
    let prev_ext_fn = vm.alloc_native_function("preventExtensions", 1, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.preventExtensions called on non-object".into())),
        };
        let ok = vm.prevent_extensions_val(target)?;
        Ok(JsValue::from_boolean(ok))
    });
    vm.set_property(reflect_obj, "preventExtensions", prev_ext_fn);

    // 28.1.12 Reflect.set ( target, propertyKey, V [ , receiver ] )
    let set_fn = vm.alloc_native_function("set", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.set called on non-object".into())),
        };
        let prop_key = match args.get(1) {
            Some(p) => vm.to_property_key(*p)?,
            None => PropertyKeyVal::String("undefined".to_string()),
        };
        let val = args.get(2).copied().unwrap_or(JsValue::undefined());
        let receiver = args.get(3).copied().unwrap_or(target);

        match prop_key {
            PropertyKeyVal::String(s) => {
                let ok = vm.set_property_with_receiver(target, &s, val, receiver)?;
                Ok(JsValue::from_boolean(ok))
            }
            PropertyKeyVal::Symbol(sym) => {
                let ok = vm.set_symbol_property_with_receiver(target, sym, val, receiver)?;
                Ok(JsValue::from_boolean(ok))
            }
        }
    });
    vm.set_property(reflect_obj, "set", set_fn);

    // 28.1.13 Reflect.setPrototypeOf ( target, proto )
    let set_proto_fn = vm.alloc_native_function("setPrototypeOf", 2, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_js_object() => *t,
            _ => return Err(VmError::TypeError("Reflect.setPrototypeOf called on non-object".into())),
        };
        let proto = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !proto.is_js_object() && !proto.is_null() {
            return Err(VmError::TypeError("Object prototype may only be an Object or null".into()));
        }
        if vm.module_namespace_objects.contains(&target) {
            return Ok(JsValue::from_boolean(proto.is_null()));
        }
        let ok = vm.set_prototype_of_val(target, proto)?;
        Ok(JsValue::from_boolean(ok))
    });
    vm.set_property(reflect_obj, "setPrototypeOf", set_proto_fn);

    if let Some(tag_sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("Reflect".into());
        vm.set_symbol_property(reflect_obj, tag_sym, tag_val);
        let key_name = format!("[Symbol:{:x}]", tag_sym.raw_bits());
        vm.non_writable_props.insert((reflect_obj, key_name.clone()));
        vm.non_enumerable_props.insert((reflect_obj, key_name));
    }

    vm.globals.insert("Reflect".to_string(), reflect_obj);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Reflect", reflect_obj);
    }
}
