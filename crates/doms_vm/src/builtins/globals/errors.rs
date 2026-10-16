//! The `Error` hierarchy (`TypeError`, `RangeError`, …).

use super::*;

pub(super) fn register_error_types(vm: &mut Vm) {
    let error_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if !this.is_object() {
            return Err(VmError::TypeError("Error.prototype.toString called on non-object".into()));
        }
        let name_val = vm.get_property(this, "name");
        let name = if name_val.is_undefined() {
            "Error".to_string()
        } else {
            let prim = vm.to_primitive(name_val, crate::interpreter::PreferredType::String)?;
            vm.to_string_value(prim)
        };
        let msg_val = vm.get_property(this, "message");
        let msg = if msg_val.is_undefined() {
            "".to_string()
        } else {
            let prim = vm.to_primitive(msg_val, crate::interpreter::PreferredType::String)?;
            vm.to_string_value(prim)
        };
        if name.is_empty() {
            Ok(vm.alloc_string(msg))
        } else if msg.is_empty() {
            Ok(vm.alloc_string(name))
        } else {
            Ok(vm.alloc_string(format!("{}: {}", name, msg)))
        }
    });

    let error_types = ["Error", "TypeError", "RangeError", "ReferenceError", "SyntaxError", "URIError", "EvalError", "AggregateError"];
    let mut error_proto = JsValue::undefined();
    let mut error_ctor = JsValue::undefined();
    for err_type in error_types {
        let err_name = err_type.to_string();
        let proto = vm.alloc_object();
        let name_val = vm.alloc_string(err_name.clone());
        vm.set_property(proto, "name", name_val);
        vm.non_enumerable_props.insert((proto, "name".to_string()));

        let empty_msg = vm.alloc_string("".to_string());
        vm.set_property(proto, "message", empty_msg);
        vm.non_enumerable_props.insert((proto, "message".to_string()));

        if err_type == "Error" {
            error_proto = proto;
            vm.set_property(proto, "toString", error_to_string);
            vm.non_enumerable_props.insert((proto, "toString".to_string()));
        } else if let Some(ptr) = proto.as_object_ptr() {
            let proto_obj = unsafe { &mut *(ptr as *mut JsObject) };
            proto_obj.prototype = Some(error_proto);
        }

        let err_name_c = err_name.clone();
        let length = if err_type == "AggregateError" { 2 } else { 1 };
        let ctor = vm.alloc_closure_native_function(err_type, length, Arc::new(move |vm, _this, args| {
            let (msg_arg, opts_arg) = if err_name_c == "AggregateError" {
                (args.get(1).copied(), args.get(2).copied())
            } else {
                (args.first().copied(), args.get(1).copied())
            };
            let msg = if let Some(arg) = msg_arg {
                if !arg.is_undefined() {
                    if arg.is_symbol() {
                        return Err(VmError::TypeError("Cannot convert a Symbol value to a string".into()));
                    }
                    let prim = vm.to_primitive(arg, crate::interpreter::PreferredType::String)?;
                    if prim.is_symbol() {
                        return Err(VmError::TypeError("Cannot convert a Symbol value to a string".into()));
                    }
                    Some(vm.to_string_value(prim))
                } else {
                    None
                }
            } else {
                None
            };
            let err_val = vm.alloc_error_opt(err_name_c.clone(), msg);
            if err_name_c == "AggregateError" {
                let mut elements = Vec::new();
                if let Some(errs_arg) = args.first().copied() {
                    if let Some(ptr) = errs_arg.as_object_ptr() {
                        let obj = unsafe { &*(ptr as *mut JsObject) };
                        if let ObjectKind::Array(arr) = &obj.kind {
                            elements = arr.clone();
                        } else {
                            // Try iterating via Symbol.iterator
                            let iter_res = (|| -> Result<Vec<JsValue>, VmError> {
                                let sym_iter = vm.symbol_iterator.ok_or_else(|| VmError::TypeError("No Symbol.iterator".into()))?;
                                let method = vm.get_symbol_property_result(errs_arg, sym_iter)?;
                                if !vm.is_callable(method) {
                                    return Err(VmError::TypeError("Symbol.iterator is not callable".into()));
                                }
                                let it = vm.call(method, errs_arg, &[])?;
                                let next_fn = vm.get_property(it, "next");
                                let mut res = Vec::new();
                                loop {
                                    let step = vm.call(next_fn, it, &[])?;
                                    let done = vm.get_property(step, "done");
                                    if done.is_truthy() {
                                        break;
                                    }
                                    let val = vm.get_property(step, "value");
                                    res.push(val);
                                }
                                Ok(res)
                            })();
                            match iter_res {
                                Ok(items) => elements = items,
                                Err(e) => return Err(e),
                            }
                        }
                    } else {
                        return Err(VmError::TypeError("AggregateError errors argument is not iterable".into()));
                    }
                }
                let errors_arr = vm.alloc_array(elements);
                vm.set_property(err_val, "errors", errors_arr);
                vm.non_enumerable_props.insert((err_val, "errors".to_string()));
            }
            if let Some(opts) = opts_arg {
                if opts.is_object() {
                    let has_cause = vm.has_property(opts, "cause")?;
                    if has_cause {
                        let cause = vm.get_property_result(opts, "cause")?;
                        vm.set_property(err_val, "cause", cause);
                        vm.non_enumerable_props.insert((err_val, "cause".to_string()));
                    }
                }
            }
            Ok(err_val)
        }));

        if err_type == "Error" {
            error_ctor = ctor;
        } else if error_ctor.is_object() {
            if let Some(ptr) = ctor.as_object_ptr() {
                let ctor_obj = unsafe { &mut *(ptr as *mut JsObject) };
                ctor_obj.prototype = Some(error_ctor);
            }
        }

        vm.set_property(proto, "constructor", ctor);
        vm.non_enumerable_props.insert((proto, "constructor".to_string()));
        vm.set_property(ctor, "prototype", proto);
        vm.non_enumerable_props.insert((ctor, "prototype".to_string()));
        vm.non_writable_props.insert((ctor, "prototype".to_string()));
        vm.non_configurable_props.insert((ctor, "prototype".to_string()));
        vm.globals.insert(err_type.to_string(), ctor);
    }

    if error_ctor.is_object() {
        let is_error_fn = vm.alloc_native_function("isError", 1, |_vm, _this, args| {
            if let Some(arg) = args.first() {
                if let Some(ptr) = arg.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    return Ok(JsValue::from_boolean(matches!(obj.kind, ObjectKind::Error { .. })));
                }
            }
            Ok(JsValue::from_boolean(false))
        });
        vm.set_property(error_ctor, "isError", is_error_fn);
    }
}
