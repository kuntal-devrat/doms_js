//! The `Promise` constructor, statics and prototype methods.

use super::*;

mod combinators;
mod constructor;
pub(super) mod iterator;
mod prototype;
mod settled;
mod statics;

pub(super) fn register_promise(vm: &mut Vm) {
    let promise_ctor = constructor::create_constructor(vm);

    let mut promise_statics = statics::create_basic_statics(vm);
    promise_statics.extend(combinators::create_combinator_statics(vm));
    promise_statics.extend(settled::create_settled_statics(vm));
    for (name, fn_val) in promise_statics {
        vm.set_property(promise_ctor, name, fn_val);
    }

    let promise_proto = prototype::create_prototype(vm, promise_ctor);

    vm.set_property(promise_ctor, "prototype", promise_proto);
    vm.non_enumerable_props.insert((promise_ctor, "prototype".to_string()));
    add_species_getter(vm, promise_ctor);

    vm.globals.insert("Promise".to_string(), promise_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Promise", promise_ctor);
    }
}

#[derive(Clone, Copy)]
pub(super) struct PromiseCapability {
    pub promise: JsValue,
    pub resolve: JsValue,
    pub reject: JsValue,
}

pub(super) fn new_promise_capability(vm: &mut Vm, ctor: JsValue) -> Result<PromiseCapability, VmError> {
    if !vm.is_constructor(ctor) {
        return Err(VmError::TypeError("Promise constructor expected".into()));
    }
    let res_holder = Arc::new(std::sync::Mutex::new(JsValue::undefined()));
    let rej_holder = Arc::new(std::sync::Mutex::new(JsValue::undefined()));
    let res_c = Arc::clone(&res_holder);
    let rej_c = Arc::clone(&rej_holder);

    let executor = vm.alloc_closure_native_function("", 2, Arc::new(move |_vm, _this, args| {
        let mut r_guard = res_c.lock().unwrap();
        let mut j_guard = rej_c.lock().unwrap();
        if !r_guard.is_undefined() || !j_guard.is_undefined() {
            return Err(VmError::TypeError("Promise capability executor called more than once".into()));
        }
        let r = args.first().copied().unwrap_or(JsValue::undefined());
        let j = args.get(1).copied().unwrap_or(JsValue::undefined());
        *r_guard = r;
        *j_guard = j;
        Ok(JsValue::undefined())
    }));

    let promise = vm.construct(ctor, &[executor])?;
    let resolve = *res_holder.lock().unwrap();
    let reject = *rej_holder.lock().unwrap();

    if !vm.is_callable(resolve) || !vm.is_callable(reject) {
        return Err(VmError::TypeError("Promise capability executor did not set callable resolve and reject".into()));
    }

    Ok(PromiseCapability { promise, resolve, reject })
}
