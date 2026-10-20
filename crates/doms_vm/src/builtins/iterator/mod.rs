//! The `Iterator` built-in object and prototype hierarchy.

use crate::interpreter::{Vm, VmError};
use crate::object::{
    ArrayIterationKind as IterKind, JsObject, MapIterationKind, ObjectKind, SetIterationKind,
};
use crate::value::JsValue;

pub mod concat;
pub mod from;
pub mod helper;
pub mod prototype;
pub mod wrapper;
pub mod zip;

use concat::iterator_concat;
use from::iterator_from;
use helper::register_iterator_helper_proto;
use prototype::register_iterator_prototype_methods;
use wrapper::register_wrap_for_valid_iterator_proto;
use zip::iterator_zip;

pub fn register_iterator_builtins(vm: &mut Vm) {
    // 1. %IteratorPrototype%
    let iter_proto = vm.alloc_object();
    vm.iterator_proto = Some(iter_proto);

    // 2. %WrapForValidIteratorPrototype%
    let _wrap_proto = register_wrap_for_valid_iterator_proto(vm, iter_proto);

    // 3. %IteratorHelperPrototype%
    let _helper_proto = register_iterator_helper_proto(vm, iter_proto);

    // 4. %ArrayIteratorPrototype%
    let array_iter_proto = vm.alloc_object_with_exact_proto(Some(iter_proto));
    vm.array_iterator_proto = Some(array_iter_proto);

    let array_iter_next = vm.alloc_native_function("next", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::ArrayIterator { target, index, kind } = &mut obj.kind {
                let cur_target = *target;
                if let Some(tptr) = cur_target.as_object_ptr() {
                    let tobj = unsafe { &*(tptr as *mut JsObject) };
                    if let ObjectKind::TypedArray(ta) = &tobj.kind {
                        if ta.buffer.is_detached() {
                            return Err(VmError::TypeError("TypedArray buffer is detached".into()));
                        }
                    }
                }
                let cur_kind = *kind;
                let curr_idx = *index;
                let len = vm.get_property(cur_target, "length").to_number().unwrap_or(0.0) as usize;
                if curr_idx < len {
                    *index += 1;
                    let val = match cur_kind {
                        IterKind::Keys => JsValue::from_int32(curr_idx as i32),
                        IterKind::Values => vm.get_element_result(cur_target, curr_idx)?,
                        IterKind::Entries => {
                            let el = vm.get_element_result(cur_target, curr_idx)?;
                            let prev = vm.temp_roots.len();
                            vm.temp_roots.push(el);
                            let arr = vm.alloc_array(vec![
                                JsValue::from_int32(curr_idx as i32),
                                vm.temp_roots[prev],
                            ]);
                            vm.temp_roots.truncate(prev);
                            arr
                        }
                    };
                    return vm.alloc_iter_result(val, false);
                } else {
                    return vm.alloc_iter_result(JsValue::undefined(), true);
                }
            }
        }
        Err(VmError::TypeError(
            "%ArrayIteratorPrototype%.next called on incompatible receiver".into(),
        ))
    });
    vm.set_property(array_iter_proto, "next", array_iter_next);
    vm.set_property_non_enumerable(array_iter_proto, "next".to_string());

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Array Iterator".into());
        vm.set_symbol_property(array_iter_proto, sym_tag, tag_str);
        let key_name = format!("[Symbol:{:x}]", sym_tag.raw_bits());
        vm.set_property_non_enumerable(array_iter_proto, key_name.clone());
        vm.set_property_non_writable(array_iter_proto, key_name);
    }

    // 5. %MapIteratorPrototype%
    let map_iter_proto = vm.alloc_object_with_exact_proto(Some(iter_proto));
    vm.map_iterator_proto = Some(map_iter_proto);

    let map_iter_next = vm.alloc_native_function("next", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::MapIterator { map, index, kind } = &mut obj.kind {
                let map_val = *map;
                let cur_kind = *kind;
                if let Some(map_ptr) = map_val.as_object_ptr() {
                    let map_obj = unsafe { &*(map_ptr as *mut JsObject) };
                    if let ObjectKind::Map(entries) = &map_obj.kind {
                        while *index < entries.len() {
                            let entry = entries[*index];
                            *index += 1;
                            if let Some((k, v)) = entry {
                                let result_val = match cur_kind {
                                    MapIterationKind::Key => k,
                                    MapIterationKind::Value => v,
                                    MapIterationKind::KeyValue => vm.alloc_array(vec![k, v]),
                                };
                                return vm.alloc_iter_result(result_val, false);
                            }
                        }
                    }
                }
                return vm.alloc_iter_result(JsValue::undefined(), true);
            }
        }
        Err(VmError::TypeError(
            "%MapIteratorPrototype%.next called on incompatible receiver".into(),
        ))
    });
    vm.set_property(map_iter_proto, "next", map_iter_next);
    vm.set_property_non_enumerable(map_iter_proto, "next".to_string());

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Map Iterator".into());
        vm.set_symbol_property(map_iter_proto, sym_tag, tag_str);
        let key_name = format!("[Symbol:{:x}]", sym_tag.raw_bits());
        vm.set_property_non_enumerable(map_iter_proto, key_name.clone());
        vm.set_property_non_writable(map_iter_proto, key_name);
    }

    // 6. %SetIteratorPrototype%
    let set_iter_proto = vm.alloc_object_with_exact_proto(Some(iter_proto));
    vm.set_iterator_proto = Some(set_iter_proto);

    let set_iter_next = vm.alloc_native_function("next", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::SetIterator { set, index, kind } = &mut obj.kind {
                let set_val = *set;
                let cur_kind = *kind;
                if let Some(set_ptr) = set_val.as_object_ptr() {
                    let set_obj = unsafe { &*(set_ptr as *mut JsObject) };
                    if let ObjectKind::Set(entries) = &set_obj.kind {
                        while *index < entries.len() {
                            let entry = entries[*index];
                            *index += 1;
                            if let Some(v) = entry {
                                let result_val = match cur_kind {
                                    SetIterationKind::Value => v,
                                    SetIterationKind::KeyValue => vm.alloc_array(vec![v, v]),
                                };
                                return vm.alloc_iter_result(result_val, false);
                            }
                        }
                    }
                }
                return vm.alloc_iter_result(JsValue::undefined(), true);
            }
        }
        Err(VmError::TypeError(
            "%SetIteratorPrototype%.next called on incompatible receiver".into(),
        ))
    });
    vm.set_property(set_iter_proto, "next", set_iter_next);
    vm.set_property_non_enumerable(set_iter_proto, "next".to_string());

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Set Iterator".into());
        vm.set_symbol_property(set_iter_proto, sym_tag, tag_str);
        let key_name = format!("[Symbol:{:x}]", sym_tag.raw_bits());
        vm.set_property_non_enumerable(set_iter_proto, key_name.clone());
        vm.set_property_non_writable(set_iter_proto, key_name);
    }

    // 7. %StringIteratorPrototype%
    let string_iter_proto = vm.alloc_object_with_exact_proto(Some(iter_proto));
    vm.string_iterator_proto = Some(string_iter_proto);

    let string_iter_next = vm.alloc_native_function("next", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::StringIterator { string, index } = &mut obj.kind {
                if *index >= string.len() {
                    return vm.alloc_iter_result(JsValue::undefined(), true);
                }
                let s = &string[*index..];
                if let Some(ch) = s.chars().next() {
                    *index += ch.len_utf8();
                    let char_str = vm.alloc_string(ch.to_string());
                    return vm.alloc_iter_result(char_str, false);
                } else {
                    return vm.alloc_iter_result(JsValue::undefined(), true);
                }
            }
        }
        Err(VmError::TypeError(
            "%StringIteratorPrototype%.next called on incompatible receiver".into(),
        ))
    });
    vm.set_property(string_iter_proto, "next", string_iter_next);
    vm.set_property_non_enumerable(string_iter_proto, "next".to_string());

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("String Iterator".into());
        vm.set_symbol_property(string_iter_proto, sym_tag, tag_str);
        let key_name = format!("[Symbol:{:x}]", sym_tag.raw_bits());
        vm.set_property_non_enumerable(string_iter_proto, key_name.clone());
        vm.set_property_non_writable(string_iter_proto, key_name);
    }

    // 8. %GeneratorPrototype%
    let gen_proto = vm.alloc_object_with_exact_proto(Some(iter_proto));
    vm.generator_proto = Some(gen_proto);
    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Generator".into());
        vm.set_symbol_property(gen_proto, sym_tag, tag_str);
        let key_name = format!("[Symbol:{:x}]", sym_tag.raw_bits());
        vm.set_property_non_enumerable(gen_proto, key_name.clone());
        vm.set_property_non_writable(gen_proto, key_name);
    }

    // 9. Iterator Constructor
    let iter_ctor = vm.alloc_native_function("Iterator", 0, |vm, this, _args| {
        if !vm.is_constructing {
            return Err(VmError::TypeError(
                "Iterator constructor cannot be invoked without 'new'".into(),
            ));
        }
        let iter_ctor_val = vm.globals.get("Iterator").copied().unwrap_or(JsValue::undefined());
        let new_target = vm.current_new_target;
        if new_target.is_none() || new_target == Some(iter_ctor_val) {
            return Err(VmError::TypeError(
                "Iterator is an abstract constructor and cannot be directly instantiated".into(),
            ));
        }
        Ok(this)
    });

    // Iterator.prototype
    vm.set_property(iter_ctor, "prototype", iter_proto);
    vm.set_property_non_enumerable(iter_ctor, "prototype".to_string());
    vm.set_property_non_writable(iter_ctor, "prototype".to_string());
    vm.set_property_non_configurable(iter_ctor, "prototype".to_string());

    // Register prototype methods on iter_proto
    register_iterator_prototype_methods(vm, iter_proto, iter_ctor);

    // Static method: Iterator.from
    let from_fn = vm.alloc_native_function("from", 1, |vm, _this, args| {
        let o = args.first().copied().unwrap_or(JsValue::undefined());
        iterator_from(vm, o)
    });
    vm.set_property(iter_ctor, "from", from_fn);
    vm.set_property_non_enumerable(iter_ctor, "from".to_string());

    // Static method: Iterator.concat
    let concat_fn =
        vm.alloc_native_function("concat", 0, |vm, _this, args| iterator_concat(vm, args));
    vm.set_property(iter_ctor, "concat", concat_fn);
    vm.set_property_non_enumerable(iter_ctor, "concat".to_string());

    // Static method: Iterator.zip
    let zip_fn = vm.alloc_native_function("zip", 1, |vm, _this, args| iterator_zip(vm, args));
    vm.set_property(iter_ctor, "zip", zip_fn);
    vm.set_property_non_enumerable(iter_ctor, "zip".to_string());

    // Global binding: Iterator
    vm.globals.insert("Iterator".to_string(), iter_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "Iterator", iter_ctor);
        vm.set_property_non_enumerable(vm.global_object, "Iterator".to_string());
    }
}
