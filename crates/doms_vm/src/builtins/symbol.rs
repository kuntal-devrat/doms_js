//! The `Symbol` global object and well-known symbols.

use super::*;

pub(super) fn register_symbol(vm: &mut Vm) {
    let symbol_proto = vm.alloc_object();
    vm.symbol_proto = Some(symbol_proto);

    let sym_value_of = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if this.is_symbol() {
            return Ok(this);
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Symbol(_)) {
                return Ok(unsafe { JsValue::from_symbol_ptr(ptr) });
            }
        }
        Err(VmError::TypeError("Symbol.prototype.valueOf requires that 'this' be a Symbol".into()))
    });
    let sym_to_string = vm.alloc_native_function("toString", 0, |vm, this, _args| {
        if this.is_symbol() {
            return Ok(vm.alloc_string(vm.to_string_value(this)));
        }
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Symbol(desc) = &obj.kind {
                return Ok(vm.alloc_string(format!("Symbol({})", desc.as_deref().unwrap_or(""))));
            }
        }
        Err(VmError::TypeError("Symbol.prototype.toString requires that 'this' be a Symbol".into()))
    });
    vm.set_property(symbol_proto, "valueOf", sym_value_of);
    vm.set_property(symbol_proto, "toString", sym_to_string);

    let symbol_ctor = vm.alloc_native_function("Symbol", 1, |vm, _this, args| {
        if vm.is_constructing {
            return Err(VmError::TypeError("Symbol is not a constructor".into()));
        }
        let desc = args.first().map(|v| vm.to_string_value(*v));
        Ok(vm.alloc_symbol(desc))
    });
    vm.set_property(symbol_proto, "constructor", symbol_ctor);
    vm.non_enumerable_props.insert((symbol_proto, "constructor".to_string()));
    vm.set_property(symbol_ctor, "prototype", symbol_proto);
    vm.non_enumerable_props.insert((symbol_ctor, "prototype".to_string()));
    vm.non_writable_props.insert((symbol_ctor, "prototype".to_string()));
    vm.non_configurable_props.insert((symbol_ctor, "prototype".to_string()));

    let symbol_for = vm.alloc_native_function("for", 1, |vm, _this, args| {
        let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        if let Some(existing) = vm.symbol_registry.get(&key) {
            return Ok(*existing);
        }
        let sym = vm.alloc_symbol(Some(key.clone()));
        vm.symbol_registry.insert(key, sym);
        Ok(sym)
    });
    vm.set_property(symbol_ctor, "for", symbol_for);

    let symbol_key_for = vm.alloc_native_function("keyFor", 1, |vm, _this, args| {
        let sym = args.first().copied().unwrap_or(JsValue::undefined());
        if !sym.is_symbol() {
            return Err(VmError::TypeError("Symbol.keyFor requires that argument be a Symbol".into()));
        }
        for (k, v) in &vm.symbol_registry {
            if *v == sym {
                return Ok(vm.alloc_string(k.clone()));
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(symbol_ctor, "keyFor", symbol_key_for);

    // Standard well-known symbols
    let sym_to_prim = vm.alloc_symbol(Some("Symbol.toPrimitive".into()));
    let sym_iter = vm.alloc_symbol(Some("Symbol.iterator".into()));
    let sym_async_iter = vm.alloc_symbol(Some("Symbol.asyncIterator".into()));
    let sym_has_instance = vm.alloc_symbol(Some("Symbol.hasInstance".into()));
    let sym_concat_spread = vm.alloc_symbol(Some("Symbol.isConcatSpreadable".into()));
    let sym_match = vm.alloc_symbol(Some("Symbol.match".into()));
    let sym_match_all = vm.alloc_symbol(Some("Symbol.matchAll".into()));
    let sym_replace = vm.alloc_symbol(Some("Symbol.replace".into()));
    let sym_search = vm.alloc_symbol(Some("Symbol.search".into()));
    let sym_species = vm.alloc_symbol(Some("Symbol.species".into()));
    let sym_split = vm.alloc_symbol(Some("Symbol.split".into()));
    let sym_tag = vm.alloc_symbol(Some("Symbol.toStringTag".into()));
    let sym_unscopables = vm.alloc_symbol(Some("Symbol.unscopables".into()));

    vm.set_property(symbol_ctor, "toPrimitive", sym_to_prim);
    vm.set_property(symbol_ctor, "iterator", sym_iter);
    vm.set_property(symbol_ctor, "asyncIterator", sym_async_iter);
    vm.set_property(symbol_ctor, "hasInstance", sym_has_instance);
    vm.set_property(symbol_ctor, "isConcatSpreadable", sym_concat_spread);
    vm.set_property(symbol_ctor, "match", sym_match);
    vm.set_property(symbol_ctor, "matchAll", sym_match_all);
    vm.set_property(symbol_ctor, "replace", sym_replace);
    vm.set_property(symbol_ctor, "search", sym_search);
    vm.set_property(symbol_ctor, "species", sym_species);
    vm.set_property(symbol_ctor, "split", sym_split);
    vm.set_property(symbol_ctor, "toStringTag", sym_tag);
    vm.set_property(symbol_ctor, "unscopables", sym_unscopables);

    vm.symbol_iterator = Some(sym_iter);
    vm.symbol_async_iterator = Some(sym_async_iter);
    vm.symbol_to_string_tag = Some(sym_tag);
    vm.symbol_is_concat_spreadable = Some(sym_concat_spread);
    vm.symbol_species = Some(sym_species);
    vm.symbol_to_primitive = Some(sym_to_prim);
    vm.symbol_has_instance = Some(sym_has_instance);
    vm.symbol_match = Some(sym_match);
    vm.symbol_match_all = Some(sym_match_all);
    vm.symbol_replace = Some(sym_replace);
    vm.symbol_search = Some(sym_search);
    vm.symbol_split = Some(sym_split);
    vm.symbol_unscopables = Some(sym_unscopables);

    vm.globals.insert("Symbol".to_string(), symbol_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Symbol", symbol_ctor);
    }
}
