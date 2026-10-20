//! `Promise.prototype` methods (`then`, `catch`, `finally`).

use super::*;

pub(super) fn create_prototype(vm: &mut Vm, promise_ctor: JsValue) -> JsValue {
    let promise_proto = vm.alloc_object();
    vm.promise_proto = Some(promise_proto);

    let then_fn = vm.alloc_native_function("then", 2, |vm, this, args| {
        let on_fulfilled = args.first().copied().unwrap_or(JsValue::undefined());
        let on_rejected = args.get(1).copied().unwrap_or(JsValue::undefined());
        vm.promise_then(this, on_fulfilled, on_rejected)
    });
    let catch_fn = vm.alloc_native_function("catch", 1, |vm, this, args| {
        let on_rejected = args.first().copied().unwrap_or(JsValue::undefined());
        let then_val = vm.get_property_result(this, "then")?;
        vm.call(then_val, this, &[JsValue::undefined(), on_rejected])
    });
    let finally_fn = vm.alloc_native_function("finally", 1, |vm, this, args| {
        let on_finally = args.first().copied().unwrap_or(JsValue::undefined());
        let then_val = vm.get_property_result(this, "then")?;
        if !vm.is_callable(then_val) {
            return Err(VmError::TypeError(
                "Promise.prototype.finally: then property is not callable".into(),
            ));
        }
        if !vm.is_callable(on_finally) {
            return vm.call(then_val, this, &[on_finally, on_finally]);
        }
        let on_fin1 = on_finally;
        let then_fin1 = vm.alloc_closure_native_function(
            "",
            1,
            Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                let val = args.first().copied().unwrap_or(JsValue::undefined());
                let res = vm.call(on_fin1, JsValue::undefined(), &[])?;
                let p_ctor = vm.globals.get("Promise").copied().unwrap_or(JsValue::undefined());
                let res_fn = vm.get_property(p_ctor, "resolve");
                let p = vm.call(res_fn, p_ctor, &[res])?;
                let ret_cb =
                    vm.alloc_closure_native_function("", 0, Arc::new(move |_vm, _, _| Ok(val)));
                let p_then = vm.get_property(p, "then");
                vm.call(p_then, p, &[ret_cb])
            }),
        );
        let on_fin2 = on_finally;
        let then_fin2 = vm.alloc_closure_native_function(
            "",
            1,
            Arc::new(move |vm: &mut Vm, _, args: &[JsValue]| -> Result<JsValue, VmError> {
                let err = args.first().copied().unwrap_or(JsValue::undefined());
                let res = vm.call(on_fin2, JsValue::undefined(), &[])?;
                let p_ctor = vm.globals.get("Promise").copied().unwrap_or(JsValue::undefined());
                let res_fn = vm.get_property(p_ctor, "resolve");
                let p = vm.call(res_fn, p_ctor, &[res])?;
                let throw_cb = vm.alloc_closure_native_function(
                    "",
                    0,
                    Arc::new(move |_vm, _, _| Err(VmError::Thrown(err))),
                );
                let p_then = vm.get_property(p, "then");
                vm.call(p_then, p, &[throw_cb])
            }),
        );
        vm.call(then_val, this, &[then_fin1, then_fin2])
    });

    vm.set_property(promise_proto, "then", then_fn);
    vm.set_property(promise_proto, "catch", catch_fn);
    vm.set_property(promise_proto, "finally", finally_fn);
    vm.set_property(promise_proto, "constructor", promise_ctor);
    vm.set_property_non_enumerable(promise_proto, "then".to_string());
    vm.set_property_non_enumerable(promise_proto, "catch".to_string());
    vm.set_property_non_enumerable(promise_proto, "finally".to_string());
    vm.set_property_non_enumerable(promise_proto, "constructor".to_string());

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("Promise".to_string());
        vm.set_symbol_property(promise_proto, sym_tag, tag_val);
        vm.set_property_non_enumerable(promise_proto, "[Symbol.toStringTag]".to_string());
    }
    promise_proto
}
