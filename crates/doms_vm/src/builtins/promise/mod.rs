//! The `Promise` constructor, statics and prototype methods.

use super::*;

mod combinators;
mod constructor;
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
