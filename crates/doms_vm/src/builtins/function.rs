//! `Function.prototype` methods (`call`, `apply`, `bind`).

use super::*;

pub(super) fn register_function_builtins(vm: &mut Vm) {
    let call_fn = vm.alloc_native_function("call", 1, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.call called on non-function".into()));
        }
        let this_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let rest_args = if args.len() > 1 { &args[1..] } else { &[] };
        vm.call(this, this_arg, rest_args)
    });

    let apply_fn = vm.alloc_native_function("apply", 2, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.apply called on non-function".into()));
        }
        let this_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let mut call_args = Vec::new();
        if let Some(arg_array) = args.get(1) {
            if !arg_array.is_nullish() {
                if let Some(ptr) = arg_array.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let ObjectKind::Array(elems) = &obj.kind {
                        call_args.extend_from_slice(elems);
                    } else {
                        let len_val = vm.get_property(*arg_array, "length");
                        let raw_len = len_val.to_number().unwrap_or(0.0);
                        if raw_len > 65536.0 || raw_len < 0.0 || raw_len.is_infinite() {
                            return Err(VmError::RangeError("Maximum call stack / argument limit exceeded".into()));
                        }
                        let len = raw_len as usize;
                        for i in 0..len {
                            call_args.push(vm.get_property(*arg_array, &i.to_string()));
                        }
                    }
                } else {
                    return Err(VmError::TypeError("CreateListFromArrayLike called on non-object".into()));
                }
            }
        }
        vm.call(this, this_arg, &call_args)
    });

    let bind_fn = vm.alloc_native_function("bind", 1, |vm, this, args| {
        if !vm.is_callable(this) {
            return Err(VmError::TypeError("Function.prototype.bind called on non-function".into()));
        }
        let target_fn = this;
        let bound_this = args.first().copied().unwrap_or(JsValue::undefined());
        let bound_args: Vec<JsValue> = if args.len() > 1 { args[1..].to_vec() } else { Vec::new() };

        let target_len = vm.get_property(target_fn, "length").to_number().unwrap_or(0.0) as usize;
        let bound_length = if target_len > bound_args.len() { target_len - bound_args.len() } else { 0 };
        let target_name = vm.get_property(target_fn, "name");
        let target_name_str = if target_name.is_string() {
            vm.to_string_value(target_name)
        } else {
            String::new()
        };
        let bound_name = format!("bound {}", target_name_str);

        let bound_closure = vm.alloc_closure_native_function(
            &bound_name,
            (bound_length.min(255)) as u8,
            Arc::new(move |vm, _call_this, call_args| {
                let mut combined_args = bound_args.clone();
                combined_args.extend_from_slice(call_args);
                if vm.is_constructing {
                    vm.construct(target_fn, &combined_args)
                } else {
                    vm.call(target_fn, bound_this, &combined_args)
                }
            }),
        );
        vm.non_enumerable_props.insert((bound_closure, "length".to_string()));
        vm.non_enumerable_props.insert((bound_closure, "name".to_string()));
        Ok(bound_closure)
    });

    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, _this, _args| {
        Ok(vm.alloc_string("function () { [native code] }".into()))
    });

    let function_proto = vm.function_proto.unwrap();

    vm.set_property(function_proto, "call", call_fn);
    vm.set_property(function_proto, "apply", apply_fn);
    vm.set_property(function_proto, "bind", bind_fn);
    vm.set_property(function_proto, "toString", to_string_fn);

    let function_constructor = vm.alloc_native_function("Function", 1, |vm, _this, args| {
        let (params_str, body_str) = if args.is_empty() {
            (String::new(), String::new())
        } else if args.len() == 1 {
            (String::new(), vm.to_string_value(args[0]))
        } else {
            let param_parts: Vec<String> = args[..args.len() - 1].iter().map(|a| vm.to_string_value(*a)).collect();
            let p = param_parts.join(", ");
            let b = vm.to_string_value(args[args.len() - 1]);
            (p, b)
        };
        let code = format!("(function({}) {{\n{}\n}})", params_str, body_str);
        let program = doms_parser::parse(&code)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let chunk = doms_compiler::compile(&program)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;
        vm.run(&chunk)
    });

    vm.set_property(function_proto, "constructor", function_constructor);
    vm.set_property(function_constructor, "prototype", function_proto);
    if let Some(ptr) = function_constructor.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.prototype = Some(function_proto);
    }

    vm.globals.insert("Function".to_string(), function_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Function", function_constructor);
    }
}
