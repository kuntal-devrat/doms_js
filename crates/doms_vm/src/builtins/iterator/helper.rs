//! %IteratorHelperPrototype% implementation.

use crate::builtins::promise::iterator::{iterator_close, IteratorRecord};
use crate::interpreter::{Vm, VmError};
use crate::object::{IteratorHelperKind, JsObject, ObjectKind};
use crate::value::JsValue;

pub fn iterator_close_normal(vm: &mut Vm, record: &IteratorRecord) -> Result<(), VmError> {
    if record.done {
        return Ok(());
    }
    let ret_method = vm.get_property_result(record.iterator, "return")?;
    if ret_method.is_undefined() || ret_method.is_null() {
        return Ok(());
    }
    if !vm.is_callable(ret_method) {
        return Err(VmError::TypeError("return method is not callable".into()));
    }
    let res = vm.call(ret_method, record.iterator, &[])?;
    if !res.is_object() {
        return Err(VmError::TypeError("Iterator return method must return an object".into()));
    }
    Ok(())
}

pub fn register_iterator_helper_proto(vm: &mut Vm, iter_proto: JsValue) -> JsValue {
    let helper_proto = vm.alloc_object_with_exact_proto(Some(iter_proto));
    vm.iterator_helper_proto = Some(helper_proto);

    let next_fn = vm.alloc_native_function("next", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::IteratorHelper { kind, iterator, next_method, done, index, executing } = &mut obj.kind {
                if *done {
                    return vm.alloc_iter_result(JsValue::undefined(), true);
                }
                if *executing {
                    return Err(VmError::TypeError("Generator is already executing".into()));
                }
                *executing = true;
                let cur_iter = *iterator;
                let cur_next = *next_method;
                let cur_idx = *index;

                let res = (|| -> Result<JsValue, VmError> {
                    match kind.as_mut() {
                        IteratorHelperKind::Map { mapper } => {
                            let cur_mapper = *mapper;
                            let step_res = vm.call(cur_next, cur_iter, &[]);
                            let step = match step_res {
                                Ok(s) => s,
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            if !step.is_object() {
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                }
                                return Err(VmError::TypeError("Iterator step must be an object".into()));
                            }
                            let step_done = match vm.get_property_result(step, "done") {
                                Ok(d) => d.is_truthy(),
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            if step_done {
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                }
                                return vm.alloc_iter_result(JsValue::undefined(), true);
                            }
                            let val = match vm.get_property_result(step, "value") {
                                Ok(v) => v,
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            let mapped = vm.call(cur_mapper, JsValue::undefined(), &[val, JsValue::from_int32(cur_idx as i32)]);
                            let mapped_val = match mapped {
                                Ok(v) => v,
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                    return Err(iterator_close(vm, &rec, Some(e)));
                                }
                            };
                            if let Some(p) = this.as_object_ptr() {
                                let o = unsafe { &mut *(p as *mut JsObject) };
                                if let ObjectKind::IteratorHelper { index: idx, .. } = &mut o.kind { *idx += 1; }
                            }
                            vm.alloc_iter_result(mapped_val, false)
                        }
                        IteratorHelperKind::Filter { predicate } => {
                            let cur_pred = *predicate;
                            loop {
                                let step_res = vm.call(cur_next, cur_iter, &[]);
                                let step = match step_res {
                                    Ok(s) => s,
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(e);
                                    }
                                };
                                if !step.is_object() {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(VmError::TypeError("Iterator step must be an object".into()));
                                }
                                let step_done = match vm.get_property_result(step, "done") {
                                    Ok(d) => d.is_truthy(),
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(e);
                                    }
                                };
                                if step_done {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return vm.alloc_iter_result(JsValue::undefined(), true);
                                }
                                let val = match vm.get_property_result(step, "value") {
                                    Ok(v) => v,
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(e);
                                    }
                                };
                                let cur_i = if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &*(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { index: idx, .. } = &o.kind { *idx } else { 0 }
                                } else { 0 };
                                let selected = vm.call(cur_pred, JsValue::undefined(), &[val, JsValue::from_int32(cur_i as i32)]);
                                let sel_val = match selected {
                                    Ok(v) => v,
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                        return Err(iterator_close(vm, &rec, Some(e)));
                                    }
                                };
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { index: idx, .. } = &mut o.kind { *idx += 1; }
                                }
                                if sel_val.is_truthy() {
                                    return vm.alloc_iter_result(val, false);
                                }
                            }
                        }
                        IteratorHelperKind::Take { remaining } => {
                            if *remaining <= 0.0 {
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                }
                                let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                iterator_close_normal(vm, &rec)?;
                                return vm.alloc_iter_result(JsValue::undefined(), true);
                            }
                            *remaining -= 1.0;
                            let step_res = vm.call(cur_next, cur_iter, &[]);
                            let step = match step_res {
                                Ok(s) => s,
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            if !step.is_object() {
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                }
                                return Err(VmError::TypeError("Iterator step must be an object".into()));
                            }
                            let step_done = match vm.get_property_result(step, "done") {
                                Ok(d) => d.is_truthy(),
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            if step_done {
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                }
                                return vm.alloc_iter_result(JsValue::undefined(), true);
                            }
                            let val = match vm.get_property_result(step, "value") {
                                Ok(v) => v,
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            vm.alloc_iter_result(val, false)
                        }
                        IteratorHelperKind::Drop { remaining } => {
                            while *remaining > 0.0 {
                                *remaining -= 1.0;
                                let step_res = vm.call(cur_next, cur_iter, &[]);
                                let step = match step_res {
                                    Ok(s) => s,
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(e);
                                    }
                                };
                                if !step.is_object() {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(VmError::TypeError("Iterator step must be an object".into()));
                                }
                                let step_done = match vm.get_property_result(step, "done") {
                                    Ok(d) => d.is_truthy(),
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(e);
                                    }
                                };
                                if step_done {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return vm.alloc_iter_result(JsValue::undefined(), true);
                                }
                            }
                            let step_res = vm.call(cur_next, cur_iter, &[]);
                            let step = match step_res {
                                Ok(s) => s,
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            if !step.is_object() {
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                }
                                return Err(VmError::TypeError("Iterator step must be an object".into()));
                            }
                            let step_done = match vm.get_property_result(step, "done") {
                                Ok(d) => d.is_truthy(),
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            if step_done {
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                }
                                return vm.alloc_iter_result(JsValue::undefined(), true);
                            }
                            let val = match vm.get_property_result(step, "value") {
                                Ok(v) => v,
                                Err(e) => {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(e);
                                }
                            };
                            vm.alloc_iter_result(val, false)
                        }
                        IteratorHelperKind::FlatMap { mapper, inner_iter, inner_next } => {
                            let cur_mapper = *mapper;
                            loop {
                                if let Some(inner) = *inner_iter {
                                    let in_next = (*inner_next).unwrap();
                                    let in_step_res = vm.call(in_next, inner, &[]);
                                    let in_step = match in_step_res {
                                        Ok(s) => s,
                                        Err(e) => {
                                            if let Some(p) = this.as_object_ptr() {
                                                let o = unsafe { &mut *(p as *mut JsObject) };
                                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                            }
                                            let in_rec = IteratorRecord { iterator: inner, next_method: in_next, done: false };
                                            let err = iterator_close(vm, &in_rec, Some(e));
                                            let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                            return Err(iterator_close(vm, &rec, Some(err)));
                                        }
                                    };
                                    if !in_step.is_object() {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        let in_rec = IteratorRecord { iterator: inner, next_method: in_next, done: false };
                                        let err = iterator_close(vm, &in_rec, Some(VmError::TypeError("Inner step must be an object".into())));
                                        let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                        return Err(iterator_close(vm, &rec, Some(err)));
                                    }
                                    let in_done = match vm.get_property_result(in_step, "done") {
                                        Ok(d) => d.is_truthy(),
                                        Err(e) => {
                                            if let Some(p) = this.as_object_ptr() {
                                                let o = unsafe { &mut *(p as *mut JsObject) };
                                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                            }
                                            let in_rec = IteratorRecord { iterator: inner, next_method: in_next, done: false };
                                            let err = iterator_close(vm, &in_rec, Some(e));
                                            let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                            return Err(iterator_close(vm, &rec, Some(err)));
                                        }
                                    };
                                    if !in_done {
                                        let in_val = match vm.get_property_result(in_step, "value") {
                                            Ok(v) => v,
                                            Err(e) => {
                                                if let Some(p) = this.as_object_ptr() {
                                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                                }
                                                let in_rec = IteratorRecord { iterator: inner, next_method: in_next, done: false };
                                                let err = iterator_close(vm, &in_rec, Some(e));
                                                let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                                return Err(iterator_close(vm, &rec, Some(err)));
                                            }
                                        };
                                        return vm.alloc_iter_result(in_val, false);
                                    }
                                    *inner_iter = None;
                                    *inner_next = None;
                                }

                                let step_res = vm.call(cur_next, cur_iter, &[]);
                                let step = match step_res {
                                    Ok(s) => s,
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(e);
                                    }
                                };
                                if !step.is_object() {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(VmError::TypeError("Iterator step must be an object".into()));
                                }
                                let step_done = match vm.get_property_result(step, "done") {
                                    Ok(d) => d.is_truthy(),
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(e);
                                    }
                                };
                                if step_done {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return vm.alloc_iter_result(JsValue::undefined(), true);
                                }
                                let val = match vm.get_property_result(step, "value") {
                                    Ok(v) => v,
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(e);
                                    }
                                };
                                let cur_i = if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &*(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { index: idx, .. } = &o.kind { *idx } else { 0 }
                                } else { 0 };
                                let mapped = vm.call(cur_mapper, JsValue::undefined(), &[val, JsValue::from_int32(cur_i as i32)]);
                                let mapped_val = match mapped {
                                    Ok(v) => v,
                                    Err(e) => {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                        return Err(iterator_close(vm, &rec, Some(e)));
                                    }
                                };
                                if let Some(p) = this.as_object_ptr() {
                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                    if let ObjectKind::IteratorHelper { index: idx, .. } = &mut o.kind { *idx += 1; }
                                }
                                if mapped_val.is_string() {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                    return Err(iterator_close(vm, &rec, Some(VmError::TypeError("flatMap mapper returned primitive string".into()))));
                                }
                                if !mapped_val.is_object() {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                    return Err(iterator_close(vm, &rec, Some(VmError::TypeError("flatMap mapper returned non-object".into()))));
                                }

                                // Get iterator flattenable
                                let sym_iter = vm.symbol_iterator;
                                let using_iter = if let Some(sym) = sym_iter {
                                    match vm.get_symbol_property_result(mapped_val, sym) {
                                        Ok(m) => {
                                            if m.is_undefined() || m.is_null() {
                                                None
                                            } else if !vm.is_callable(m) {
                                                if let Some(p) = this.as_object_ptr() {
                                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                                }
                                                let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                                return Err(iterator_close(vm, &rec, Some(VmError::TypeError("Symbol.iterator not callable".into()))));
                                            } else {
                                                Some(m)
                                            }
                                        }
                                        Err(e) => {
                                            if let Some(p) = this.as_object_ptr() {
                                                let o = unsafe { &mut *(p as *mut JsObject) };
                                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                            }
                                            let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                            return Err(iterator_close(vm, &rec, Some(e)));
                                        }
                                    }
                                } else {
                                    None
                                };

                                let (in_it, in_nx) = if let Some(m) = using_iter {
                                    let iter_obj = match vm.call(m, mapped_val, &[]) {
                                        Ok(it) => it,
                                        Err(e) => {
                                            if let Some(p) = this.as_object_ptr() {
                                                let o = unsafe { &mut *(p as *mut JsObject) };
                                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                            }
                                            let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                            return Err(iterator_close(vm, &rec, Some(e)));
                                        }
                                    };
                                    if !iter_obj.is_object() {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                        return Err(iterator_close(vm, &rec, Some(VmError::TypeError("Symbol.iterator must return an object".into()))));
                                    }
                                    let nxt = match vm.get_property_result(iter_obj, "next") {
                                        Ok(n) => n,
                                        Err(e) => {
                                            if let Some(p) = this.as_object_ptr() {
                                                let o = unsafe { &mut *(p as *mut JsObject) };
                                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                            }
                                            let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                            return Err(iterator_close(vm, &rec, Some(e)));
                                        }
                                    };
                                    (iter_obj, nxt)
                                } else {
                                    let nxt = match vm.get_property_result(mapped_val, "next") {
                                        Ok(n) => n,
                                        Err(e) => {
                                            if let Some(p) = this.as_object_ptr() {
                                                let o = unsafe { &mut *(p as *mut JsObject) };
                                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                            }
                                            let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                            return Err(iterator_close(vm, &rec, Some(e)));
                                        }
                                    };
                                    (mapped_val, nxt)
                                };

                                if !vm.is_callable(in_nx) {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                                    return Err(iterator_close(vm, &rec, Some(VmError::TypeError("inner iterator next is not callable".into()))));
                                }

                                *inner_iter = Some(in_it);
                                *inner_next = Some(in_nx);
                            }
                        }
                        IteratorHelperKind::Concat { iterables, current_idx, current_iter, current_next } => {
                            loop {
                                if let Some(in_it) = *current_iter {
                                    let in_nxt = (*current_next).unwrap();
                                    let step_res = vm.call(in_nxt, in_it, &[]);
                                    let step = match step_res {
                                        Ok(s) => s,
                                        Err(e) => {
                                            if let Some(p) = this.as_object_ptr() {
                                                let o = unsafe { &mut *(p as *mut JsObject) };
                                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                            }
                                            return Err(e);
                                        }
                                    };
                                    if !step.is_object() {
                                        if let Some(p) = this.as_object_ptr() {
                                            let o = unsafe { &mut *(p as *mut JsObject) };
                                            if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                        }
                                        return Err(VmError::TypeError("Iterator step must be an object".into()));
                                    }
                                    let step_done = match vm.get_property_result(step, "done") {
                                        Ok(d) => d.is_truthy(),
                                        Err(e) => {
                                            if let Some(p) = this.as_object_ptr() {
                                                let o = unsafe { &mut *(p as *mut JsObject) };
                                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                            }
                                            return Err(e);
                                        }
                                    };
                                    if !step_done {
                                        let val = match vm.get_property_result(step, "value") {
                                            Ok(v) => v,
                                            Err(e) => {
                                                if let Some(p) = this.as_object_ptr() {
                                                    let o = unsafe { &mut *(p as *mut JsObject) };
                                                    if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                                }
                                                return Err(e);
                                            }
                                        };
                                        return vm.alloc_iter_result(val, false);
                                    }
                                    *current_iter = None;
                                    *current_next = None;
                                }

                                if *current_idx >= iterables.len() {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return vm.alloc_iter_result(JsValue::undefined(), true);
                                }

                                let item = iterables[*current_idx];
                                *current_idx += 1;
                                let sym_iter = vm.symbol_iterator.ok_or_else(|| VmError::TypeError("Symbol.iterator not found".into()))?;
                                let iter_fn = vm.get_symbol_property_result(item, sym_iter)?;
                                let it = vm.call(iter_fn, item, &[])?;
                                if !it.is_object() {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(VmError::TypeError("Symbol.iterator must return an object".into()));
                                }
                                let nxt = vm.get_property_result(it, "next")?;
                                if !vm.is_callable(nxt) {
                                    if let Some(p) = this.as_object_ptr() {
                                        let o = unsafe { &mut *(p as *mut JsObject) };
                                        if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                                    }
                                    return Err(VmError::TypeError("next method is not callable".into()));
                                }
                                *current_iter = Some(it);
                                *current_next = Some(nxt);
                            }
                        }
                        _ => {
                            if let Some(p) = this.as_object_ptr() {
                                let o = unsafe { &mut *(p as *mut JsObject) };
                                if let ObjectKind::IteratorHelper { done: d, .. } = &mut o.kind { *d = true; }
                            }
                            vm.alloc_iter_result(JsValue::undefined(), true)
                        }
                    }
                })();

                if let Some(p) = this.as_object_ptr() {
                    let o = unsafe { &mut *(p as *mut JsObject) };
                    if let ObjectKind::IteratorHelper { executing: exec, .. } = &mut o.kind {
                        *exec = false;
                    }
                }
                return res;
            }
        }
        Err(VmError::TypeError("%IteratorHelperPrototype%.next called on incompatible receiver".into()))
    });
    vm.set_property(helper_proto, "next", next_fn);
    vm.non_enumerable_props.insert((helper_proto, "next".to_string()));

    let return_fn = vm.alloc_native_function("return", 0, |vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if let ObjectKind::IteratorHelper { kind, iterator, next_method, done, executing, .. } = &mut obj.kind {
                if *done {
                    return vm.alloc_iter_result(JsValue::undefined(), true);
                }
                if *executing {
                    return Err(VmError::TypeError("Generator is already executing".into()));
                }
                *executing = true;
                *done = true;
                let cur_iter = *iterator;
                let cur_next = *next_method;

                let close_res = (|| -> Result<(), VmError> {
                    if let IteratorHelperKind::FlatMap { inner_iter, inner_next, .. } = kind.as_mut() {
                        if let Some(inner) = *inner_iter {
                            let nx = (*inner_next).unwrap();
                            let in_rec = IteratorRecord { iterator: inner, next_method: nx, done: false };
                            iterator_close_normal(vm, &in_rec)?;
                        }
                    }
                    if let IteratorHelperKind::Concat { current_iter, .. } = kind.as_mut() {
                        if let Some(inner) = *current_iter {
                            let in_rec = IteratorRecord { iterator: inner, next_method: JsValue::undefined(), done: false };
                            iterator_close_normal(vm, &in_rec)?;
                        }
                    }

                    if cur_iter.is_object() {
                        let rec = IteratorRecord { iterator: cur_iter, next_method: cur_next, done: false };
                        iterator_close_normal(vm, &rec)?;
                    }
                    Ok(())
                })();

                if let Some(p) = this.as_object_ptr() {
                    let o = unsafe { &mut *(p as *mut JsObject) };
                    if let ObjectKind::IteratorHelper { executing: exec, .. } = &mut o.kind {
                        *exec = false;
                    }
                }
                close_res?;
                return vm.alloc_iter_result(JsValue::undefined(), true);
            }
        }
        Err(VmError::TypeError("%IteratorHelperPrototype%.return called on incompatible receiver".into()))
    });
    vm.set_property(helper_proto, "return", return_fn);
    vm.non_enumerable_props.insert((helper_proto, "return".to_string()));

    if let Some(sym_tag) = vm.symbol_to_string_tag {
        let tag_str = vm.alloc_string("Iterator Helper".into());
        vm.set_symbol_property(helper_proto, sym_tag, tag_str);
        let key_name = format!("[Symbol:{:x}]", sym_tag.raw_bits());
        vm.non_enumerable_props.insert((helper_proto, key_name.clone()));
        vm.non_writable_props.insert((helper_proto, key_name));
    }

    helper_proto
}
