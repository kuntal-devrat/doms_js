//! Typed array constructors (`Int8Array` … `BigUint64Array`).

use super::*;

fn construct_typed_array_kind(vm: &mut Vm, args: &[JsValue], kind: TypedArrayKind) -> Result<JsValue, VmError> {
    let bpe = kind.bytes_per_element();
    let arg0 = args.first().copied().unwrap_or(JsValue::undefined());
    // 1. ArrayBuffer or SharedArrayBuffer
    if let Some(ptr) = arg0.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::ArrayBuffer(sab) | ObjectKind::SharedArrayBuffer(sab) = &obj.kind {
            let buf_len = sab.byte_length();
            let byte_offset = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as usize;
            if byte_offset % bpe != 0 {
                return Err(VmError::RangeError("Start offset must be a multiple of the element size".into()));
            }
            if byte_offset > buf_len {
                return Err(VmError::RangeError("Start offset is outside the bounds of the buffer".into()));
            }
            let length = if let Some(len_arg) = args.get(2) {
                let l = len_arg.to_int32().unwrap_or(0).max(0) as usize;
                if byte_offset + l * bpe > buf_len {
                    return Err(VmError::RangeError("Invalid typed array length".into()));
                }
                l
            } else {
                (buf_len - byte_offset) / bpe
            };
            return Ok(vm.alloc_typed_array(JsTypedArray {
                buffer: Arc::clone(sab),
                kind,
                length,
                byte_offset,
            }));
        } else if let ObjectKind::TypedArray(src_ta) = &obj.kind {
            // Another TypedArray
            let len = src_ta.length;
            let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                .map_err(|e| VmError::RuntimeError(e.into()))?;
            let mut dst_ta = JsTypedArray {
                buffer: Arc::new(new_sab),
                kind,
                length: len,
                byte_offset: 0,
            };
            for i in 0..len {
                if let Some(val) = Vm::read_typed_array_element(src_ta, i) {
                    Vm::write_typed_array_element(&mut dst_ta, i, val);
                }
            }
            return Ok(vm.alloc_typed_array(dst_ta));
        } else if let ObjectKind::Array(elements) = &obj.kind {
            // Array
            let len = elements.len();
            let new_sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
                .map_err(|e| VmError::RuntimeError(e.into()))?;
            let mut dst_ta = JsTypedArray {
                buffer: Arc::new(new_sab),
                kind,
                length: len,
                byte_offset: 0,
            };
            for (i, el) in elements.iter().enumerate() {
                Vm::write_typed_array_element(&mut dst_ta, i, *el);
            }
            return Ok(vm.alloc_typed_array(dst_ta));
        }
    }

    // 2. Length or number argument
    let len = arg0.to_int32().unwrap_or(0).max(0) as usize;
    if len * bpe > 64 * 1024 * 1024 {
        return Err(VmError::RangeError("Invalid typed array length".into()));
    }
    let sab = doms_parallel::SharedArrayBuffer::try_new(len * bpe)
        .map_err(|e| VmError::RuntimeError(e.into()))?;
    Ok(vm.alloc_typed_array(JsTypedArray {
        buffer: Arc::new(sab),
        kind,
        length: len,
        byte_offset: 0,
    }))
}

fn construct_int8_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int8)
}
fn construct_uint8_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint8)
}
fn construct_uint8_clamped_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint8Clamped)
}
fn construct_int16_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int16)
}
fn construct_uint16_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint16)
}
fn construct_int32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Int32)
}
fn construct_uint32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Uint32)
}
fn construct_float32_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Float32)
}
fn construct_float64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::Float64)
}
fn construct_bigint64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    construct_typed_array_kind(vm, args, TypedArrayKind::BigInt64)
}
fn construct_biguint64_array(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
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
