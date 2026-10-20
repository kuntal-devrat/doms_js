//! Shared helpers used by several builtin registrars.

use super::*;
use std::time::Instant;

#[allow(dead_code)]
pub(super) fn get_enumerable_keys(vm: &Vm, arg: JsValue) -> Vec<String> {
    let mut raw_names: Vec<String> = Vec::new();
    if let Some(ptr) = arg.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        match &obj.kind {
            ObjectKind::Array(elements) => {
                for i in 0..elements.len() {
                    if !elements[i].is_hole() {
                        let k = i.to_string();
                        if vm.is_own_property_enumerable(arg, &k) && !raw_names.contains(&k) {
                            raw_names.push(k);
                        }
                    }
                }
            }
            ObjectKind::String(s) | ObjectKind::StringObject(s) => {
                for i in 0..s.chars().count() {
                    let k = i.to_string();
                    if !raw_names.contains(&k) {
                        raw_names.push(k);
                    }
                }
            }
            ObjectKind::TypedArray(ta) => {
                for i in 0..ta.length {
                    let k = i.to_string();
                    if !raw_names.contains(&k) {
                        raw_names.push(k);
                    }
                }
            }
            _ => {}
        }
        if let Some(accs) = &obj.accessors {
            for (k, acc) in accs.iter() {
                if acc.enumerable && !raw_names.contains(k) {
                    raw_names.push(k.clone());
                }
            }
        }
        for prop in vm.shapes.all_properties(obj.shape_id) {
            if vm.is_own_property_enumerable(arg, &prop) && !raw_names.contains(&prop) {
                raw_names.push(prop);
            }
        }
        if vm.module_namespace_objects.contains(&arg) {
            for ((ns, re_name), _) in &vm.module_reexports {
                if *ns == arg && !raw_names.contains(re_name) {
                    raw_names.push(re_name.clone());
                }
            }
            raw_names.sort();
        }
    }
    let mut int_keys: Vec<(u32, String)> = Vec::new();
    let mut str_keys: Vec<String> = Vec::new();
    for k in raw_names {
        if let Ok(idx) = k.parse::<u32>() {
            if idx < u32::MAX && idx.to_string() == k {
                int_keys.push((idx, k));
                continue;
            }
        }
        str_keys.push(k);
    }
    int_keys.sort_by_key(|(idx, _)| *idx);
    let mut ordered = Vec::new();
    for (_, k) in int_keys {
        ordered.push(k);
    }
    for k in str_keys {
        ordered.push(k);
    }
    ordered
}

pub(crate) fn add_species_getter(vm: &mut Vm, ctor: JsValue) {
    if let Some(sym_species) = vm.symbol_species {
        let species_getter =
            vm.alloc_native_function("get [Symbol.species]", 0, |_vm, this, _args| Ok(this));
        if let Some(ptr) = ctor.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.insert_symbol_accessor_getter(sym_species, species_getter);
            if let Some(accs) = obj.symbol_accessors.as_mut() {
                if let Some(acc) = accs.get_mut(&sym_species) {
                    acc.enumerable = false;
                }
            }
        }
    }
}

pub(crate) fn iterate_to_vec(vm: &mut Vm, iterable: JsValue) -> Result<Vec<JsValue>, VmError> {
    if iterable.is_nullish() {
        return Err(VmError::TypeError("Value is not iterable".into()));
    }
    if let Some(ptr) = iterable.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::Array(arr) = &obj.kind {
            let has_custom_iter = obj
                .symbol_properties
                .as_ref()
                .map_or(false, |sp| vm.symbol_iterator.map_or(false, |si| sp.contains_key(&si)));
            if !has_custom_iter {
                return Ok(arr.clone());
            }
        }
    }

    let sym_iter =
        vm.symbol_iterator.ok_or_else(|| VmError::TypeError("No Symbol.iterator".into()))?;
    let prev_temp = vm.temp_roots.len();
    vm.temp_roots.push(iterable);
    let cur_iterable = vm.temp_roots[prev_temp];
    let method = vm.get_symbol_property(cur_iterable, sym_iter);
    if !vm.is_callable(method) {
        vm.temp_roots.truncate(prev_temp);
        return Err(VmError::TypeError("Value is not iterable".into()));
    }
    vm.temp_roots.push(method);
    let cur_iterable = vm.temp_roots[prev_temp];
    let cur_method = vm.temp_roots[prev_temp + 1];
    let it = match vm.call(cur_method, cur_iterable, &[]) {
        Ok(it) => it,
        Err(e) => {
            vm.temp_roots.truncate(prev_temp);
            return Err(e);
        }
    };
    if !it.is_object() {
        vm.temp_roots.truncate(prev_temp);
        return Err(VmError::TypeError("Iterator must return an object".into()));
    }
    vm.temp_roots.push(it);
    let cur_it = vm.temp_roots[prev_temp + 2];
    let next_fn = match vm.get_property_result(cur_it, "next") {
        Ok(n) => n,
        Err(e) => {
            vm.temp_roots.truncate(prev_temp);
            return Err(e);
        }
    };
    if !vm.is_callable(next_fn) {
        vm.temp_roots.truncate(prev_temp);
        return Err(VmError::TypeError("Iterator next is not callable".into()));
    }
    vm.temp_roots.push(next_fn);

    let items_start = vm.temp_roots.len();
    loop {
        if let Some(d) = vm.deadline {
            if Instant::now() > d {
                vm.temp_roots.truncate(prev_temp);
                return Err(VmError::RuntimeError("Execution timeout exceeded".into()));
            }
        }
        if vm.temp_roots.len() - items_start >= 1_000_000 {
            vm.temp_roots.truncate(prev_temp);
            return Err(VmError::RangeError("Iterable exceeds maximum size".into()));
        }
        let cur_it = vm.temp_roots[prev_temp + 2];
        let cur_next = vm.temp_roots[prev_temp + 3];
        let step = match vm.call(cur_next, cur_it, &[]) {
            Ok(s) => s,
            Err(e) => {
                vm.temp_roots.truncate(prev_temp);
                return Err(e);
            }
        };
        if !step.is_object() {
            vm.temp_roots.truncate(prev_temp);
            return Err(VmError::TypeError("Iterator result is not an object".into()));
        }
        vm.temp_roots.push(step);
        let cur_step = vm.temp_roots.last().copied().unwrap();
        let done = match vm.get_property_result(cur_step, "done") {
            Ok(d) => d,
            Err(e) => {
                vm.temp_roots.truncate(prev_temp);
                return Err(e);
            }
        };
        if done.is_truthy() {
            vm.temp_roots.pop();
            break;
        }
        let cur_step = vm.temp_roots.last().copied().unwrap();
        let val = match vm.get_property_result(cur_step, "value") {
            Ok(v) => v,
            Err(e) => {
                vm.temp_roots.truncate(prev_temp);
                return Err(e);
            }
        };
        vm.temp_roots.pop();
        vm.temp_roots.push(val);
    }
    let res = vm.temp_roots[items_start..].to_vec();
    vm.temp_roots.truncate(prev_temp);
    Ok(res)
}

pub(super) fn define_property_on_module_namespace(
    vm: &mut Vm,
    ns: JsValue,
    key: &PropertyKeyVal,
    desc: JsValue,
) -> Result<bool, VmError> {
    match key {
        PropertyKeyVal::Symbol(sym) => {
            if vm.symbol_to_string_tag == Some(*sym) {
                let has_val = vm.has_property(desc, "value").unwrap_or(false);
                let val = vm.get_property(desc, "value");
                let has_writ = vm.has_property(desc, "writable").unwrap_or(false);
                let writ = vm.get_property(desc, "writable");
                let has_enum = vm.has_property(desc, "enumerable").unwrap_or(false);
                let en = vm.get_property(desc, "enumerable");
                let has_conf = vm.has_property(desc, "configurable").unwrap_or(false);
                let conf = vm.get_property(desc, "configurable");
                if (has_val && vm.to_string_value(val) != "Module")
                    || (has_writ && writ.is_truthy())
                    || (has_enum && en.is_truthy())
                    || (has_conf && conf.is_truthy())
                    || vm.has_property(desc, "get").unwrap_or(false)
                    || vm.has_property(desc, "set").unwrap_or(false)
                {
                    return Ok(false);
                }
                return Ok(true);
            }
            Ok(false)
        }
        PropertyKeyVal::String(name) => {
            let has_prop = if let Some(ptr) = ns.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                vm.shapes.has_property(obj.shape_id, name)
                    || vm.module_reexports.contains_key(&(ns, name.clone()))
            } else {
                false
            };
            if !has_prop {
                return Ok(false);
            }
            if vm.has_property(desc, "get").unwrap_or(false)
                || vm.has_property(desc, "set").unwrap_or(false)
            {
                return Ok(false);
            }
            if vm.has_property(desc, "configurable").unwrap_or(false)
                && vm.get_property(desc, "configurable").is_truthy()
            {
                return Ok(false);
            }
            if vm.has_property(desc, "enumerable").unwrap_or(false)
                && !vm.get_property(desc, "enumerable").is_truthy()
            {
                return Ok(false);
            }
            if vm.has_property(desc, "writable").unwrap_or(false)
                && !vm.get_property(desc, "writable").is_truthy()
            {
                return Ok(false);
            }
            if vm.has_property(desc, "value").unwrap_or(false) {
                let req_val = vm.get_property(desc, "value");
                let curr_val = vm.get_property(ns, name);
                if !vm.same_value(curr_val, req_val) {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}
