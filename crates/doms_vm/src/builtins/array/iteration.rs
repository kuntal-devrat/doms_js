//! `Array.prototype` iterator methods (`values`, `keys`, `entries` and
//! `[Symbol.iterator]`).

use super::*;

pub(super) fn register_iteration_methods(vm: &mut Vm, array_proto: JsValue) {
    let values_fn = vm.alloc_native_function("values", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Values)
    });
    let keys_fn = vm.alloc_native_function("keys", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Keys)
    });
    let entries_fn = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        create_array_iterator(vm, this, IterKind::Entries)
    });
    vm.set_property(array_proto, "values", values_fn);
    vm.set_property(array_proto, "keys", keys_fn);
    vm.set_property(array_proto, "entries", entries_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(array_proto, sym_iter, values_fn);
    }
}
