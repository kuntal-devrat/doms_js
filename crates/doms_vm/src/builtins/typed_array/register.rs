//! Typed array constructor registration and prototype hierarchy.

use super::*;

pub(super) fn typed_array_from(vm: &mut Vm, this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    if !vm.is_constructor(this) {
        return Err(VmError::TypeError("TypedArray.from called on non-constructor".into()));
    }
    let source = args.first().copied().unwrap_or(JsValue::undefined());
    let map_fn = args.get(1).copied().unwrap_or(JsValue::undefined());
    let this_arg = args.get(2).copied().unwrap_or(JsValue::undefined());
    let has_map = !map_fn.is_undefined();
    if has_map && !vm.is_callable(map_fn) {
        return Err(VmError::TypeError("TypedArray.from: when provided, mapFn must be callable".into()));
    }

    let prev_temp = vm.temp_roots.len();
    vm.temp_roots.push(this);
    vm.temp_roots.push(source);
    vm.temp_roots.push(map_fn);
    vm.temp_roots.push(this_arg);

    let cur_source = vm.temp_roots[prev_temp + 1];
    let items = if let Some(sym_iter) = vm.symbol_iterator {
        let iter_fn = vm.get_symbol_property_result(cur_source, sym_iter).unwrap_or(JsValue::undefined());
        if vm.is_callable(iter_fn) {
            helpers::iterate_to_vec(vm, cur_source)?
        } else {
            let len = vm.get_property(cur_source, "length").to_number().unwrap_or(0.0).max(0.0) as usize;
            let mut list = Vec::with_capacity(len);
            for i in 0..len {
                list.push(vm.get_element(cur_source, i));
            }
            list
        }
    } else {
        let len = vm.get_property(cur_source, "length").to_number().unwrap_or(0.0).max(0.0) as usize;
        let mut list = Vec::with_capacity(len);
        for i in 0..len {
            list.push(vm.get_element(cur_source, i));
        }
        list
    };

    let len = items.len();
    let cur_ctor = vm.temp_roots[prev_temp];
    let target = vm.construct(cur_ctor, &[JsValue::from_int32(len as i32)])?;
    let target_root = vm.temp_roots.len();
    vm.temp_roots.push(target);

    for (k, item) in items.into_iter().enumerate() {
        let cur_map = vm.temp_roots[prev_temp + 2];
        let cur_this_arg = vm.temp_roots[prev_temp + 3];
        let val = if has_map {
            vm.call(cur_map, cur_this_arg, &[item, JsValue::from_int32(k as i32)])?
        } else {
            item
        };
        let cur_target = vm.temp_roots[target_root];
        vm.set_element(cur_target, k, val);
    }

    let ret = vm.temp_roots[target_root];
    vm.temp_roots.truncate(prev_temp);
    Ok(ret)
}

pub(super) fn typed_array_of(vm: &mut Vm, this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    if !vm.is_constructor(this) {
        return Err(VmError::TypeError("TypedArray.of called on non-constructor".into()));
    }
    let len = args.len();
    let target = vm.construct(this, &[JsValue::from_int32(len as i32)])?;
    let prev_temp = vm.temp_roots.len();
    vm.temp_roots.push(target);
    for (i, &arg) in args.iter().enumerate() {
        let cur_target = vm.temp_roots[prev_temp];
        vm.set_element(cur_target, i, arg);
    }
    let res = vm.temp_roots[prev_temp];
    vm.temp_roots.truncate(prev_temp);
    Ok(res)
}

/// Register every typed array constructor (`Int8Array` … `BigUint64Array`) and
/// the `%TypedArray%` super-constructor hierarchy.
pub(crate) fn register_typed_arrays(vm: &mut Vm) {
    // 1. %TypedArray% constructor
    let typed_array_ctor = vm.alloc_native_function("TypedArray", 0, |_vm, _this, _args| {
        Err(VmError::TypeError("Abstract class %TypedArray% cannot be instantiated directly".into()))
    });
    // Set prototype of %TypedArray% to Function.prototype
    if let Some(fn_proto) = vm.function_proto {
        if let Some(ptr) = typed_array_ctor.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = Some(fn_proto);
        }
    }

    // 2. %TypedArray%.prototype
    let typed_array_proto = vm.alloc_object();
    // Set prototype of %TypedArray%.prototype to Object.prototype
    if let Some(obj_proto) = vm.object_proto {
        if let Some(ptr) = typed_array_proto.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = Some(obj_proto);
        }
    }

    // Connect %TypedArray% <-> %TypedArray%.prototype
    vm.set_property(typed_array_ctor, "prototype", typed_array_proto);
    vm.set_property_non_enumerable(typed_array_ctor, "prototype".to_string());
    vm.set_property_non_writable(typed_array_ctor, "prototype".to_string());
    vm.set_property_non_configurable(typed_array_ctor, "prototype".to_string());

    vm.set_property(typed_array_proto, "constructor", typed_array_ctor);
    vm.set_property_non_enumerable(typed_array_proto, "constructor".to_string());

    // 3. Register methods and getters on %TypedArray%.prototype
    methods::register_prototype_methods(vm, typed_array_proto);

    // 4. Register static methods on %TypedArray%
    let from_fn = vm.alloc_native_function("from", 1, typed_array_from);
    let of_fn = vm.alloc_native_function("of", 0, typed_array_of);
    vm.set_property(typed_array_ctor, "from", from_fn);
    vm.set_property_non_enumerable(typed_array_ctor, "from".to_string());
    vm.set_property(typed_array_ctor, "of", of_fn);
    vm.set_property_non_enumerable(typed_array_ctor, "of".to_string());
    helpers::add_species_getter(vm, typed_array_ctor);

    // 5. Register each concrete typed array constructor
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
        let name = kind.name();
        let bpe = kind.bytes_per_element();
        let ctor_fn: NativeFn = constructors::ctor_fn_for(kind);

        let ctor = vm.alloc_native_function(name, 3, ctor_fn);
        let proto = vm.alloc_object();

        // Object.setPrototypeOf(ctor, TypedArray)
        if let Some(ptr) = ctor.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = Some(typed_array_ctor);
        }

        // Object.setPrototypeOf(proto, TypedArray.prototype)
        if let Some(ptr) = proto.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = Some(typed_array_proto);
        }

        vm.set_property(ctor, "BYTES_PER_ELEMENT", JsValue::from_int32(bpe as i32));
        vm.set_property_non_enumerable(ctor, "BYTES_PER_ELEMENT".to_string());
        vm.set_property_non_writable(ctor, "BYTES_PER_ELEMENT".to_string());
        vm.set_property_non_configurable(ctor, "BYTES_PER_ELEMENT".to_string());

        vm.set_property(proto, "BYTES_PER_ELEMENT", JsValue::from_int32(bpe as i32));
        vm.set_property_non_enumerable(proto, "BYTES_PER_ELEMENT".to_string());
        vm.set_property_non_writable(proto, "BYTES_PER_ELEMENT".to_string());
        vm.set_property_non_configurable(proto, "BYTES_PER_ELEMENT".to_string());

        vm.set_property(proto, "constructor", ctor);
        vm.set_property_non_enumerable(proto, "constructor".to_string());

        vm.set_property(ctor, "prototype", proto);
        vm.set_property_non_enumerable(ctor, "prototype".to_string());
        vm.set_property_non_writable(ctor, "prototype".to_string());
        vm.set_property_non_configurable(ctor, "prototype".to_string());

        vm.globals.insert(name.to_string(), ctor);
        if vm.global_object.is_object() {
            vm.set_property(vm.global_object, name, ctor);
        }
    }
}
