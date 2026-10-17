//! `Array` constructor, statics and `Array.prototype`.

use super::*;

mod callbacks;
mod constructor;
mod iteration;
mod prototype;
mod statics;

pub(super) fn register_array_builtins(vm: &mut Vm) {
    let (array_constructor, is_array_fn) = constructor::register_constructor(vm);
    let array_proto = prototype::register_prototype_methods(vm);
    callbacks::register_callback_methods(vm, array_proto);
    iteration::register_iteration_methods(vm, array_proto);
    statics::register_statics(vm, array_constructor);

    vm.set_property(array_proto, "constructor", array_constructor);

    let proto_methods = [
        "push", "pop", "shift", "unshift", "slice", "splice", "indexOf", "lastIndexOf",
        "includes", "join", "concat", "reverse", "sort", "forEach", "map", "filter",
        "reduce", "reduceRight", "some", "every", "find", "findIndex", "findLast",
        "findLastIndex", "flat", "flatMap", "at", "toReversed", "toSorted", "toSpliced",
        "with", "entries", "keys", "values", "constructor", "toString", "toLocaleString",
    ];
    for m in proto_methods {
        vm.non_enumerable_props.insert((array_proto, m.to_string()));
    }

    vm.set_property(array_constructor, "isArray", is_array_fn);
    vm.set_property(array_constructor, "prototype", array_proto);
    for s in ["isArray", "from", "of", "prototype"] {
        vm.non_enumerable_props.insert((array_constructor, s.to_string()));
    }
    vm.non_writable_props.insert((array_constructor, "prototype".to_string()));
    vm.non_configurable_props.insert((array_constructor, "prototype".to_string()));

    add_species_getter(vm, array_constructor);
    vm.globals.insert("Array".to_string(), array_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Array", array_constructor);
    }
}
