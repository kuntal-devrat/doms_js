//! The `Object` constructor, its statics and `Object.prototype`.

use super::*;

mod constructor;
pub(crate) mod define_property;
pub(crate) mod descriptor;
mod group_by;
pub(crate) mod integrity;
mod legacy_accessors;
pub(crate) mod names;
mod predicates;
pub(crate) mod prototype;
pub(crate) mod statics;

use constructor::register_constructor;
use define_property::register_define_property;
use descriptor::register_descriptor;
use group_by::register_group_by;
use integrity::register_integrity;
use legacy_accessors::register_legacy_accessors;
use names::register_names_and_symbols;
use predicates::register_predicates;
use prototype::register_prototype_methods;
use statics::register_statics;

pub(super) fn register_object_builtins(vm: &mut Vm) {
    let (object_proto, object_constructor, keys_fn, values_fn, entries_fn, assign_fn) =
        register_constructor(vm);
    register_prototype_methods(vm, object_proto);
    register_legacy_accessors(vm, object_proto);
    register_predicates(vm, object_proto);
    let group_by_fn = register_group_by(vm);
    let define_property_fn = register_define_property(vm);
    let (get_own_prop_desc_fn, get_own_prop_descriptors_fn) = register_descriptor(vm);
    let (get_own_prop_names_fn, get_own_prop_symbols_fn) = register_names_and_symbols(vm);
    let (
        prevent_extensions_fn,
        is_extensible_fn,
        freeze_fn,
        is_frozen_fn,
        seal_fn,
        is_sealed_fn,
    ) = register_integrity(vm);
    let (get_prototype_of_fn, set_prototype_of_fn, has_own_fn, from_entries_fn, is_fn, create_fn, define_properties_fn) =
        register_statics(vm);

    vm.set_property(object_constructor, "is", is_fn);
    vm.set_property(object_constructor, "create", create_fn);
    vm.set_property(object_constructor, "defineProperties", define_properties_fn);

    vm.set_property(object_constructor, "defineProperty", define_property_fn);
    vm.set_property(object_constructor, "getOwnPropertyDescriptor", get_own_prop_desc_fn);
    vm.set_property(object_constructor, "getOwnPropertyDescriptors", get_own_prop_descriptors_fn);
    vm.set_property(object_constructor, "getOwnPropertyNames", get_own_prop_names_fn);
    vm.set_property(object_constructor, "getOwnPropertySymbols", get_own_prop_symbols_fn);
    vm.set_property(object_constructor, "preventExtensions", prevent_extensions_fn);
    vm.set_property(object_constructor, "isExtensible", is_extensible_fn);
    vm.set_property(object_constructor, "freeze", freeze_fn);
    vm.set_property(object_constructor, "isFrozen", is_frozen_fn);
    vm.set_property(object_constructor, "seal", seal_fn);
    vm.set_property(object_constructor, "isSealed", is_sealed_fn);
    vm.set_property(object_constructor, "getPrototypeOf", get_prototype_of_fn);
    vm.set_property(object_constructor, "setPrototypeOf", set_prototype_of_fn);
    vm.set_property(object_constructor, "hasOwn", has_own_fn);
    vm.set_property(object_constructor, "fromEntries", from_entries_fn);
    vm.set_property(object_constructor, "keys", keys_fn);
    vm.set_property(object_constructor, "values", values_fn);
    vm.set_property(object_constructor, "entries", entries_fn);
    vm.set_property(object_constructor, "assign", assign_fn);
    vm.set_property(object_constructor, "groupBy", group_by_fn);
    vm.set_property(object_constructor, "prototype", object_proto);

    let statics = [
        "is", "create", "defineProperties", "defineProperty",
        "getOwnPropertyDescriptor", "getOwnPropertyDescriptors",
        "getOwnPropertyNames", "getOwnPropertySymbols",
        "preventExtensions", "isExtensible", "freeze", "isFrozen",
        "seal", "isSealed", "getPrototypeOf", "setPrototypeOf",
        "hasOwn", "fromEntries", "keys", "values", "entries",
        "assign", "groupBy", "prototype",
    ];
    for name in statics {
        vm.non_enumerable_props.insert((object_constructor, name.to_string()));
    }
    vm.non_writable_props.insert((object_constructor, "prototype".to_string()));
    vm.non_configurable_props.insert((object_constructor, "prototype".to_string()));

    vm.globals.insert("Object".to_string(), object_constructor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Object", object_constructor);
    }
}
