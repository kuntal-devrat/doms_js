//! Typed array constructor registration.

use super::*;

pub(super) fn register_single_typed_array(vm: &mut Vm, kind: TypedArrayKind) {
    let name = kind.name();
    let bpe = kind.bytes_per_element();

    let ctor_fn: NativeFn = constructors::ctor_fn_for(kind);

    let ctor = vm.alloc_native_function(name, 1, ctor_fn);

    let proto = vm.alloc_object();

    methods::register_prototype_methods(vm, proto);

    vm.set_property(ctor, "BYTES_PER_ELEMENT", JsValue::from_int32(bpe as i32));
    vm.set_property(proto, "BYTES_PER_ELEMENT", JsValue::from_int32(bpe as i32));
    vm.set_property(proto, "constructor", ctor);
    vm.set_property(ctor, "prototype", proto);

    vm.globals.insert(name.to_string(), ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, name, ctor);
    }
}

/// Register every typed array constructor (`Int8Array` … `BigUint64Array`).
pub(crate) fn register_typed_arrays(vm: &mut Vm) {
    let kinds = [
        TypedArrayKind::Int8,
        TypedArrayKind::Uint8,
        TypedArrayKind::Uint8Clamped,
        TypedArrayKind::Int16,
        TypedArrayKind::Uint16,
        TypedArrayKind::Int32,
        TypedArrayKind::Uint32,
        TypedArrayKind::Float32,
        TypedArrayKind::Float64,
        TypedArrayKind::BigInt64,
        TypedArrayKind::BigUint64,
    ];
    for kind in kinds {
        register_single_typed_array(vm, kind);
    }
}
