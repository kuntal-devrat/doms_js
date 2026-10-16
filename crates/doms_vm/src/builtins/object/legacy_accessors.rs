//! Legacy `Object.prototype.__lookup*__` and `__define*__` helpers.

use super::*;

pub(super) fn register_legacy_accessors(vm: &mut Vm, object_proto: JsValue) {
    let lookup_getter = vm.alloc_native_function("__lookupGetter__", 1, |vm, this, args| {
        let o = if this.is_object() { this } else { vm.to_object_value(this)? };
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        let mut curr = Some(o);
        while let Some(c_val) = curr {
            if let Some(c_ptr) = c_val.as_object_ptr() {
                let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                match &key {
                    PropertyKeyVal::Symbol(sym) => {
                        if let Some(accs) = &c_obj.symbol_accessors {
                            if let Some(acc) = accs.get(sym) {
                                return Ok(acc.getter.unwrap_or(JsValue::undefined()));
                            }
                        }
                        if let Some(syms) = &c_obj.symbol_properties {
                            if syms.contains_key(sym) {
                                return Ok(JsValue::undefined());
                            }
                        }
                    }
                    PropertyKeyVal::String(s) => {
                        if let Some(accs) = &c_obj.accessors {
                            if let Some(acc) = accs.get(s) {
                                return Ok(acc.getter.unwrap_or(JsValue::undefined()));
                            }
                        }
                        if vm.shapes.has_property(c_obj.shape_id, s) {
                            return Ok(JsValue::undefined());
                        }
                    }
                }
                curr = c_obj.prototype;
            } else {
                break;
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(object_proto, "__lookupGetter__", lookup_getter);
    vm.non_enumerable_props.insert((object_proto, "__lookupGetter__".to_string()));

    let lookup_setter = vm.alloc_native_function("__lookupSetter__", 1, |vm, this, args| {
        let o = if this.is_object() { this } else { vm.to_object_value(this)? };
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        let mut curr = Some(o);
        while let Some(c_val) = curr {
            if let Some(c_ptr) = c_val.as_object_ptr() {
                let c_obj = unsafe { &*(c_ptr as *mut JsObject) };
                match &key {
                    PropertyKeyVal::Symbol(sym) => {
                        if let Some(accs) = &c_obj.symbol_accessors {
                            if let Some(acc) = accs.get(sym) {
                                return Ok(acc.setter.unwrap_or(JsValue::undefined()));
                            }
                        }
                        if let Some(syms) = &c_obj.symbol_properties {
                            if syms.contains_key(sym) {
                                return Ok(JsValue::undefined());
                            }
                        }
                    }
                    PropertyKeyVal::String(s) => {
                        if let Some(accs) = &c_obj.accessors {
                            if let Some(acc) = accs.get(s) {
                                return Ok(acc.setter.unwrap_or(JsValue::undefined()));
                            }
                        }
                        if vm.shapes.has_property(c_obj.shape_id, s) {
                            return Ok(JsValue::undefined());
                        }
                    }
                }
                curr = c_obj.prototype;
            } else {
                break;
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(object_proto, "__lookupSetter__", lookup_setter);
    vm.non_enumerable_props.insert((object_proto, "__lookupSetter__".to_string()));

    let define_getter_fn = vm.alloc_native_function("__defineGetter__", 2, |vm, this, args| {
        let o = if this.is_object() { this } else { vm.to_object_value(this)? };
        let getter = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(getter) {
            return Err(VmError::TypeError("Getter must be a function".into()));
        }
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        if let Some(ptr) = o.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match key {
                PropertyKeyVal::Symbol(sym) => {
                    let accs = obj.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    accs.entry(sym).or_default().getter = Some(getter);
                }
                PropertyKeyVal::String(s) => {
                    obj.define_getter(s, getter);
                }
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(object_proto, "__defineGetter__", define_getter_fn);
    vm.non_enumerable_props.insert((object_proto, "__defineGetter__".to_string()));

    let define_setter_fn = vm.alloc_native_function("__defineSetter__", 2, |vm, this, args| {
        let o = if this.is_object() { this } else { vm.to_object_value(this)? };
        let setter = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(setter) {
            return Err(VmError::TypeError("Setter must be a function".into()));
        }
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        if let Some(ptr) = o.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            match key {
                PropertyKeyVal::Symbol(sym) => {
                    let accs = obj.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                    accs.entry(sym).or_default().setter = Some(setter);
                }
                PropertyKeyVal::String(s) => {
                    obj.define_setter(s, setter);
                }
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(object_proto, "__defineSetter__", define_setter_fn);
    vm.non_enumerable_props.insert((object_proto, "__defineSetter__".to_string()));
}
