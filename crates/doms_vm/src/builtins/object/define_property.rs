//! `Object.defineProperty`.

use super::*;

pub(super) fn register_define_property(vm: &mut Vm) -> JsValue {
    vm.alloc_native_function("defineProperty", 3, |vm, _this, args| {
        let target = match args.first() {
            Some(t) if t.is_object() => *t,
            _ => return Err(VmError::TypeError("Object.defineProperty called on non-object".into())),
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
            if success {
                return Ok(target);
            } else {
                return Err(VmError::TypeError("Cannot redefine property on module namespace object".into()));
            }
        }

        let get_val = vm.get_property(desc, "get");
        let set_val = vm.get_property(desc, "set");
        let has_get = !get_val.is_undefined();
        let has_set = !set_val.is_undefined();

        let key_name = match &prop_key {
            PropertyKeyVal::String(s) => s.clone(),
            PropertyKeyVal::Symbol(sym) => format!("[Symbol:{:x}]", sym.raw_bits()),
        };

        let is_non_configurable = vm.non_configurable_props.contains(&(target, key_name.clone()));
        if is_non_configurable {
            let conf_val = vm.get_property(desc, "configurable");
            if !conf_val.is_undefined() && conf_val.is_truthy() {
                return Err(VmError::TypeError("Cannot redefine property: configurable is false".into()));
            }
            let is_non_writable = vm.non_writable_props.contains(&(target, key_name.clone()));
            if is_non_writable {
                let writ_val = vm.get_property(desc, "writable");
                if !writ_val.is_undefined() && writ_val.is_truthy() {
                    return Err(VmError::TypeError("Cannot redefine property: writable is false".into()));
                }
            }
        }

        match prop_key {
            PropertyKeyVal::Symbol(sym) => {
                if has_get || has_set {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        let accs = obj.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                        let entry = accs.entry(sym).or_insert(Accessor { getter: None, setter: None });
                        if has_get {
                            if !vm.is_callable(get_val) {
                                return Err(VmError::TypeError("Getter must be a function".into()));
                            }
                            entry.getter = Some(get_val);
                        }
                        if has_set {
                            if !vm.is_callable(set_val) {
                                return Err(VmError::TypeError("Setter must be a function".into()));
                            }
                            entry.setter = Some(set_val);
                        }
                        let order = obj.symbol_order.get_or_insert_with(Vec::new);
                        if !order.contains(&sym) {
                            order.push(sym);
                        }
                    }
                } else {
                    let has_val = vm.has_property(desc, "value").unwrap_or(false);
                    if has_val || !vm.has_own_property_key(target, &PropertyKeyVal::Symbol(sym)) {
                        let val = vm.get_property(desc, "value");
                        if let Some(ptr) = target.as_object_ptr() {
                            let obj = unsafe { &mut *(ptr as *mut JsObject) };
                            obj.insert_symbol_property(sym, val);
                        }
                    }
                }
            }
            PropertyKeyVal::String(ref prop_name) => {
                if is_non_configurable {
                    let has_accessor = if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        obj.accessors.as_ref().map_or(false, |accs| accs.contains_key(prop_name))
                    } else {
                        false
                    };
                    if has_accessor && (vm.has_property(desc, "value").unwrap_or(false) || vm.has_property(desc, "writable").unwrap_or(false)) {
                        return Err(VmError::TypeError("Cannot redefine accessor property as data property".into()));
                    }
                    if !has_accessor && (has_get || has_set) {
                        return Err(VmError::TypeError("Cannot redefine data property as accessor property".into()));
                    }
                    let is_non_writable = vm.non_writable_props.contains(&(target, prop_name.clone()));
                    if is_non_writable {
                        if vm.has_property(desc, "value").unwrap_or(false) {
                            let new_val = vm.get_property(desc, "value");
                            let curr_val = vm.get_property(target, &prop_name);
                            if !vm.same_value(curr_val, new_val) {
                                return Err(VmError::TypeError("Cannot change value of non-configurable non-writable property".into()));
                            }
                        }
                    }
                }
                if has_get || has_set {
                    if let Some(ptr) = target.as_object_ptr() {
                        let obj = unsafe { &mut *(ptr as *mut JsObject) };
                        if has_get {
                            if !vm.is_callable(get_val) {
                                return Err(VmError::TypeError("Getter must be a function".into()));
                            }
                            obj.define_getter(prop_name.clone(), get_val);
                        }
                        if has_set {
                            if !vm.is_callable(set_val) {
                                return Err(VmError::TypeError("Setter must be a function".into()));
                            }
                            obj.define_setter(prop_name.clone(), set_val);
                        }
                    }
                } else {
                    let has_val = vm.has_property(desc, "value").unwrap_or(false);
                    if has_val || !vm.has_own_property(target, &prop_name) {
                        let val = vm.get_property(desc, "value");
                        vm.set_own_property(target, &prop_name, val);
                        if target == vm.global_object {
                            vm.globals.insert(prop_name.clone(), val);
                        }
                    }
                }
            }
        }

        let conf_val = vm.get_property(desc, "configurable");
        if !conf_val.is_undefined() {
            let is_conf = conf_val.is_truthy();
            if !is_conf {
                vm.non_configurable_props.insert((target, key_name.clone()));
            } else {
                vm.non_configurable_props.remove(&(target, key_name.clone()));
            }
        }

        let enum_val = vm.get_property(desc, "enumerable");
        if !enum_val.is_undefined() {
            let is_enum = enum_val.is_truthy();
            if !is_enum {
                vm.non_enumerable_props.insert((target, key_name.clone()));
            } else {
                vm.non_enumerable_props.remove(&(target, key_name.clone()));
            }
        }

        let writ_val = vm.get_property(desc, "writable");
        if !writ_val.is_undefined() {
            let is_writ = writ_val.is_truthy();
            if !is_writ {
                vm.non_writable_props.insert((target, key_name.clone()));
                if let PropertyKeyVal::String(prop_name) = &prop_key {
                    vm.sync_mapped_arguments_delete(target, prop_name);
                }
            } else {
                vm.non_writable_props.remove(&(target, key_name.clone()));
            }
        }

        Ok(target)
    })
}
