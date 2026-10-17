//! The `Object` constructor, its core statics and the creation of
//! `Object.prototype`.

use super::*;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum EnumerableKind {
    Key,
    Value,
    KeyValue,
}

pub fn enumerable_own_property_names(
    vm: &mut Vm,
    obj: JsValue,
    kind: EnumerableKind,
) -> Result<Vec<JsValue>, VmError> {
    let own_keys = super::names::get_own_property_keys(vm, obj)?;
    let mut properties = Vec::new();
    for key in own_keys {
        if let PropertyKeyVal::String(k_str) = key {
            let desc = super::descriptor::get_own_property_descriptor_internal(
                vm,
                obj,
                &PropertyKeyVal::String(k_str.clone()),
            )?;
            if desc.is_object() {
                let enum_val = vm.get_property(desc, "enumerable");
                if enum_val.is_truthy() {
                    match kind {
                        EnumerableKind::Key => {
                            properties.push(vm.alloc_string(k_str));
                        }
                        EnumerableKind::Value => {
                            let val = vm.get_property_result(obj, &k_str)?;
                            properties.push(val);
                        }
                        EnumerableKind::KeyValue => {
                            let val = vm.get_property_result(obj, &k_str)?;
                            let k_val = vm.alloc_string(k_str);
                            let entry = vm.alloc_array(vec![k_val, val]);
                            properties.push(entry);
                        }
                    }
                }
            }
        }
    }
    Ok(properties)
}

pub(super) fn register_constructor(
    vm: &mut Vm,
) -> (JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) {
    let object_constructor = vm.alloc_native_function("Object", 1, |vm, _this, args| {
        if let Some(arg) = args.first() {
            if !arg.is_nullish() {
                return vm.to_object_value(*arg);
            }
        }
        Ok(vm.alloc_object())
    });

    let keys_fn = vm.alloc_native_function("keys", 1, |vm, _this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let arg = vm.to_object_value(val)?;
        let items = enumerable_own_property_names(vm, arg, EnumerableKind::Key)?;
        Ok(vm.alloc_array(items))
    });

    let values_fn = vm.alloc_native_function("values", 1, |vm, _this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let arg = vm.to_object_value(val)?;
        let items = enumerable_own_property_names(vm, arg, EnumerableKind::Value)?;
        Ok(vm.alloc_array(items))
    });

    let entries_fn = vm.alloc_native_function("entries", 1, |vm, _this, args| {
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let arg = vm.to_object_value(val)?;
        let items = enumerable_own_property_names(vm, arg, EnumerableKind::KeyValue)?;
        Ok(vm.alloc_array(items))
    });

    let assign_fn = vm.alloc_native_function("assign", 2, |vm, _this, args| {
        let target_raw = args.first().copied().unwrap_or(JsValue::undefined());
        let to = vm.to_object_value(target_raw)?;
        if args.len() <= 1 {
            return Ok(to);
        }

        let saved_strict = vm.is_strict;
        vm.is_strict = true;

        let res = (|| -> Result<(), VmError> {
            for source in args.iter().skip(1) {
                if source.is_nullish() {
                    continue;
                }
                let from = vm.to_object_value(*source)?;
                let keys = super::names::get_own_property_keys(vm, from)?;
                for key in keys {
                    let desc = super::descriptor::get_own_property_descriptor_internal(vm, from, &key)?;
                    if desc.is_object() {
                        let enum_val = vm.get_property(desc, "enumerable");
                        if enum_val.is_truthy() {
                            let val = match &key {
                                PropertyKeyVal::String(s) => vm.get_property_result(from, s)?,
                                PropertyKeyVal::Symbol(sym) => vm.get_symbol_property_result(from, *sym)?,
                            };
                            match &key {
                                PropertyKeyVal::String(s) => vm.set_property_result(to, s, val)?,
                                PropertyKeyVal::Symbol(sym) => vm.set_symbol_property_result(to, *sym, val)?,
                            }
                        }
                    }
                }
            }
            Ok(())
        })();

        vm.is_strict = saved_strict;
        res?;
        Ok(to)
    });

    let object_proto = vm.alloc_object();
    vm.object_proto = Some(object_proto);
    if let Some(ptr) = vm.global_object.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.prototype = Some(object_proto);
    }

    vm.set_property(object_proto, "constructor", object_constructor);
    vm.non_enumerable_props.insert((object_proto, "constructor".to_string()));
    (object_proto, object_constructor, keys_fn, values_fn, entries_fn, assign_fn)
}
