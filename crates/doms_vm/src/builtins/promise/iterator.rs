//! ECMA-262 compliant iterator operations for Promise combinators.

use crate::value::JsValue;
use crate::{Vm, VmError};

pub struct IteratorRecord {
    pub iterator: JsValue,
    pub next_method: JsValue,
    pub done: bool,
}

pub fn get_iterator(vm: &mut Vm, iterable: JsValue) -> Result<IteratorRecord, VmError> {
    if !iterable.is_object() {
        return Err(VmError::TypeError("Value is not iterable".into()));
    }
    let sym_iter = vm
        .symbol_iterator
        .ok_or_else(|| VmError::TypeError("No Symbol.iterator".into()))?;
    let method = vm.get_symbol_property_result(iterable, sym_iter)?;
    if !vm.is_callable(method) {
        return Err(VmError::TypeError("Value is not iterable".into()));
    }
    let iterator = vm.call(method, iterable, &[])?;
    if !iterator.is_object() {
        return Err(VmError::TypeError("Iterator is not an object".into()));
    }
    let next_method = vm.get_property_result(iterator, "next")?;
    if !vm.is_callable(next_method) {
        return Err(VmError::TypeError("Iterator next is not callable".into()));
    }
    Ok(IteratorRecord {
        iterator,
        next_method,
        done: false,
    })
}

pub fn iterator_step(
    vm: &mut Vm,
    record: &mut IteratorRecord,
) -> Result<Option<JsValue>, VmError> {
    let result = match vm.call(record.next_method, record.iterator, &[]) {
        Ok(res) => res,
        Err(e) => {
            record.done = true;
            return Err(e);
        }
    };
    if !result.is_object() {
        record.done = true;
        return Err(VmError::TypeError("Iterator result is not an object".into()));
    }
    let done = match vm.get_property_result(result, "done") {
        Ok(d) => d,
        Err(e) => {
            record.done = true;
            return Err(e);
        }
    };
    if done.is_truthy() {
        record.done = true;
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

pub fn iterator_value(
    vm: &mut Vm,
    record: &mut IteratorRecord,
    step_result: JsValue,
) -> Result<JsValue, VmError> {
    match vm.get_property_result(step_result, "value") {
        Ok(v) => Ok(v),
        Err(e) => {
            record.done = true;
            Err(e)
        }
    }
}

pub fn iterator_close(
    vm: &mut Vm,
    record: &IteratorRecord,
    completion_err: Option<VmError>,
) -> VmError {
    if record.done {
        return completion_err
            .unwrap_or_else(|| VmError::TypeError("Iterator already completed".into()));
    }
    let ret_method: JsValue = match vm.get_property_result(record.iterator, "return") {
        Ok(m) => m,
        Err(e) => return completion_err.unwrap_or(e),
    };
    if ret_method.is_undefined() || ret_method.is_null() {
        return completion_err
            .unwrap_or_else(|| VmError::TypeError("Iterator return is undefined".into()));
    }
    if !vm.is_callable(ret_method) {
        return completion_err
            .unwrap_or_else(|| VmError::TypeError("Iterator return is not callable".into()));
    }
    let inner_result = vm.call(ret_method, record.iterator, &[]);
    if let Err(inner_err) = inner_result {
        return inner_err;
    }
    if let Some(err) = completion_err {
        return err;
    }
    match inner_result {
        Ok(v) => {
            if !v.is_object() {
                VmError::TypeError("Iterator return did not return an object".into())
            } else {
                VmError::TypeError("Iterator closed".into())
            }
        }
        Err(e) => e,
    }
}
