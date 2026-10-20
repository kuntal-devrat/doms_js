//! Typed array constructors (`Int8Array` … `BigUint64Array`).

use super::*;

pub(super) fn construct_typed_array_kind(
    vm: &mut Vm,
    args: &[JsValue],
    kind: TypedArrayKind,
) -> Result<JsValue, VmError> {
    if !vm.is_constructing {
        return Err(VmError::TypeError(format!("Constructor {} requires 'new'", kind.name())));
    }
    let bpe = kind.bytes_per_element();
    if args.is_empty() || args[0].is_undefined() {
        let sab = Arc::new(
            doms_parallel::SharedArrayBuffer::try_new(0)
                .map_err(|e| VmError::RuntimeError(e.into()))?,
        );
        let buf_val = vm.alloc_array_buffer(Arc::clone(&sab));
        return Ok(vm.alloc_typed_array(JsTypedArray {
            buffer: sab,
            buffer_val: Some(buf_val),
            kind,
            length: 0,
            byte_offset: 0,
        }));
    }
    let arg0 = args[0];

    // 1. If arg0 is Object
    if let Some(ptr) = arg0.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::ArrayBuffer(sab) | ObjectKind::SharedArrayBuffer(sab) = &obj.kind {
            let sab = Arc::clone(sab);
            let byte_offset =
                if let Some(&off_val) = args.get(1) { vm.to_index(off_val)? } else { 0 };
            if byte_offset % bpe != 0 {
                return Err(VmError::RangeError(
                    "Start offset must be a multiple of the element size".into(),
                ));
            }
            let length = if let Some(&len_arg) = args.get(2) {
                if len_arg.is_undefined() { None } else { Some(vm.to_index(len_arg)?) }
            } else {
                None
            };
            if sab.is_detached() {
                return Err(VmError::TypeError("ArrayBuffer is detached".into()));
            }
            let buf_len = sab.byte_length();
            let length = match length {
                None => {
                    if byte_offset > buf_len || (buf_len - byte_offset) % bpe != 0 {
                        return Err(VmError::RangeError("Invalid typed array length".into()));
                    }
                    (buf_len - byte_offset) / bpe
                }
                Some(len) => {
                    if byte_offset + len * bpe > buf_len {
                        return Err(VmError::RangeError("Invalid typed array length".into()));
                    }
                    len
                }
            };
            return Ok(vm.alloc_typed_array(JsTypedArray {
                buffer: sab,
                buffer_val: Some(arg0),
                kind,
                length,
                byte_offset,
            }));
        } else if let ObjectKind::TypedArray(src_ta) = &obj.kind {
            let src_ta = src_ta.clone();
            let len = src_ta.length;
            let new_sab = Arc::new(
                doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                    .map_err(|e| VmError::RuntimeError(e.into()))?,
            );
            let buf_val = vm.alloc_array_buffer(Arc::clone(&new_sab));
            let mut dst_ta = JsTypedArray {
                buffer: new_sab,
                buffer_val: Some(buf_val),
                kind,
                length: len,
                byte_offset: 0,
            };
            for i in 0..len {
                if let Some(val) = vm.read_typed_array_element_vm(&src_ta, i) {
                    vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
                }
            }
            return Ok(vm.alloc_typed_array(dst_ta));
        }

        // Check if iterable via Symbol.iterator
        let is_iterable = if let Some(sym_iter) = vm.symbol_iterator {
            let iter_fn =
                vm.get_symbol_property_result(arg0, sym_iter).unwrap_or(JsValue::undefined());
            vm.is_callable(iter_fn)
        } else {
            false
        };

        if is_iterable {
            let items = helpers::iterate_to_vec(vm, arg0)?;
            let len = items.len();
            let new_sab = Arc::new(
                doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                    .map_err(|e| VmError::RuntimeError(e.into()))?,
            );
            let buf_val = vm.alloc_array_buffer(Arc::clone(&new_sab));
            let mut dst_ta = JsTypedArray {
                buffer: new_sab,
                buffer_val: Some(buf_val),
                kind,
                length: len,
                byte_offset: 0,
            };
            for (i, val) in items.into_iter().enumerate() {
                vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
            }
            return Ok(vm.alloc_typed_array(dst_ta));
        }

        // Array-like object with length property
        let len_val = vm.get_property(arg0, "length");
        if !len_val.is_undefined() {
            let len = vm.to_length(len_val)?;
            let new_sab = Arc::new(
                doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                    .map_err(|e| VmError::RuntimeError(e.into()))?,
            );
            let buf_val = vm.alloc_array_buffer(Arc::clone(&new_sab));
            let mut dst_ta = JsTypedArray {
                buffer: new_sab,
                buffer_val: Some(buf_val),
                kind,
                length: len,
                byte_offset: 0,
            };
            let prev_temp = vm.temp_roots.len();
            vm.temp_roots.push(arg0);
            for i in 0..len {
                let cur_src = vm.temp_roots[prev_temp];
                let val = vm.get_element(cur_src, i);
                vm.write_typed_array_element_vm(&mut dst_ta, i, val)?;
            }
            vm.temp_roots.truncate(prev_temp);
            return Ok(vm.alloc_typed_array(dst_ta));
        }
    }

    // 2. Length or number argument
    let len = vm.to_index(arg0)?;
    let sab = Arc::new(
        doms_parallel::SharedArrayBuffer::try_new(len * bpe)
            .map_err(|e| VmError::RuntimeError(e.into()))?,
    );
    let buf_val = vm.alloc_array_buffer(Arc::clone(&sab));
    Ok(vm.alloc_typed_array(JsTypedArray {
        buffer: sab,
        buffer_val: Some(buf_val),
        kind,
        length: len,
        byte_offset: 0,
    }))
}

fn construct_int8_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int8)
}
fn construct_uint8_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint8)
}
fn construct_uint8_clamped_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint8Clamped)
}
fn construct_int16_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int16)
}
fn construct_uint16_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint16)
}
fn construct_int32_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int32)
}
fn construct_uint32_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint32)
}
fn construct_float32_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Float32)
}
fn construct_float64_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Float64)
}
fn construct_bigint64_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::BigInt64)
}
fn construct_biguint64_array(
    vm: &mut Vm,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::BigUint64)
}

pub(super) fn ctor_fn_for(kind: TypedArrayKind) -> NativeFn {
    match kind {
        TypedArrayKind::Int8 => construct_int8_array,
        TypedArrayKind::Uint8 => construct_uint8_array,
        TypedArrayKind::Uint8Clamped => construct_uint8_clamped_array,
        TypedArrayKind::Int16 => construct_int16_array,
        TypedArrayKind::Uint16 => construct_uint16_array,
        TypedArrayKind::Int32 => construct_int32_array,
        TypedArrayKind::Uint32 => construct_uint32_array,
        TypedArrayKind::Float32 => construct_float32_array,
        TypedArrayKind::Float64 => construct_float64_array,
        TypedArrayKind::BigInt64 => construct_bigint64_array,
        TypedArrayKind::BigUint64 => construct_biguint64_array,
    }
}
