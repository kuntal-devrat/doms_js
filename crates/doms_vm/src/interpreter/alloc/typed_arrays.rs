//! Allocation and element I/O for SharedArrayBuffers, ArrayBuffers,
//! DataViews and typed arrays.

use super::*;

impl Vm {
    pub fn alloc_shared_array_buffer(&mut self, sab: Arc<doms_parallel::SharedArrayBuffer>) -> JsValue {
        let obj = JsObject::new_shared_array_buffer(ShapeRegistry::ROOT_SHAPE_ID, None, sab);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_typed_array(&mut self, typed_array: JsTypedArray) -> JsValue {
        let proto = self.globals.get(typed_array.kind.name()).copied().map(|ctor| self.get_property(ctor, "prototype"));
        let obj = JsObject::new_typed_array(ShapeRegistry::ROOT_SHAPE_ID, proto, typed_array);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_array_buffer(&mut self, buffer: Arc<doms_parallel::SharedArrayBuffer>) -> JsValue {
        let proto = self.globals.get("ArrayBuffer").copied().map(|ctor| self.get_property(ctor, "prototype"));
        let obj = JsObject::new_array_buffer(ShapeRegistry::ROOT_SHAPE_ID, proto, buffer);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn alloc_data_view(&mut self, buffer: Arc<doms_parallel::SharedArrayBuffer>, byte_offset: usize, byte_length: usize) -> JsValue {
        let proto = self.globals.get("DataView").copied().map(|ctor| self.get_property(ctor, "prototype"));
        let obj = JsObject::new_data_view(ShapeRegistry::ROOT_SHAPE_ID, proto, buffer, byte_offset, byte_length);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
    pub fn read_typed_array_element(ta: &JsTypedArray, index: usize) -> Option<JsValue> {
        if ta.buffer.is_detached() || index >= ta.length {
            return None;
        }
        let bpe = ta.kind.bytes_per_element();
        let off = ta.byte_offset + index * bpe;
        if off + bpe > ta.buffer.byte_length() {
            return None;
        }
        match ta.kind {
            TypedArrayKind::Int8 => {
                let b = ta.buffer.get(off)? as i8;
                Some(JsValue::from_int32(b as i32))
            }
            TypedArrayKind::Uint8 | TypedArrayKind::Uint8Clamped => {
                let b = ta.buffer.get(off)?;
                Some(JsValue::from_int32(b as i32))
            }
            TypedArrayKind::Int16 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let val = i16::from_le_bytes([b0, b1]);
                Some(JsValue::from_int32(val as i32))
            }
            TypedArrayKind::Uint16 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let val = u16::from_le_bytes([b0, b1]);
                Some(JsValue::from_int32(val as i32))
            }
            TypedArrayKind::Int32 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let b2 = ta.buffer.get(off + 2)?;
                let b3 = ta.buffer.get(off + 3)?;
                let val = i32::from_le_bytes([b0, b1, b2, b3]);
                Some(JsValue::from_int32(val))
            }
            TypedArrayKind::Uint32 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let b2 = ta.buffer.get(off + 2)?;
                let b3 = ta.buffer.get(off + 3)?;
                let val = u32::from_le_bytes([b0, b1, b2, b3]);
                Some(JsValue::from_double(val as f64))
            }
            TypedArrayKind::Float32 => {
                let b0 = ta.buffer.get(off)?;
                let b1 = ta.buffer.get(off + 1)?;
                let b2 = ta.buffer.get(off + 2)?;
                let b3 = ta.buffer.get(off + 3)?;
                let val = f32::from_le_bytes([b0, b1, b2, b3]);
                Some(JsValue::from_double(val as f64))
            }
            TypedArrayKind::Float64 => {
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = ta.buffer.get(off + i)?;
                }
                let val = f64::from_le_bytes(bytes);
                Some(JsValue::from_double(val))
            }
            TypedArrayKind::BigInt64 => {
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = ta.buffer.get(off + i)?;
                }
                let val = i64::from_le_bytes(bytes);
                Some(JsValue::from_double(val as f64))
            }
            TypedArrayKind::BigUint64 => {
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = ta.buffer.get(off + i)?;
                }
                let val = u64::from_le_bytes(bytes);
                Some(JsValue::from_double(val as f64))
            }
        }
    }
    pub fn write_typed_array_element(ta: &JsTypedArray, index: usize, value: JsValue) -> bool {
        if ta.buffer.is_detached() || index >= ta.length {
            return false;
        }
        let bpe = ta.kind.bytes_per_element();
        let off = ta.byte_offset + index * bpe;
        if off + bpe > ta.buffer.byte_length() {
            return false;
        }
        let num = value.to_number().unwrap_or(0.0);
        match ta.kind {
            TypedArrayKind::Int8 => {
                let b = (num as i32 as i8) as u8;
                ta.buffer.set(off, b);
            }
            TypedArrayKind::Uint8 => {
                let b = num as i32 as u8;
                ta.buffer.set(off, b);
            }
            TypedArrayKind::Uint8Clamped => {
                let clamped = if num.is_nan() || num <= 0.0 {
                    0u8
                } else if num >= 255.0 {
                    255u8
                } else {
                    let floor = num.floor();
                    let diff = num - floor;
                    if diff < 0.5 {
                        floor as u8
                    } else if diff > 0.5 {
                        (floor + 1.0) as u8
                    } else if (floor as u64) % 2 == 0 {
                        floor as u8
                    } else {
                        (floor + 1.0) as u8
                    }
                };
                ta.buffer.set(off, clamped);
            }
            TypedArrayKind::Int16 => {
                let bytes = (num as i32 as i16).to_le_bytes();
                ta.buffer.set(off, bytes[0]);
                ta.buffer.set(off + 1, bytes[1]);
            }
            TypedArrayKind::Uint16 => {
                let bytes = (num as u32 as u16).to_le_bytes();
                ta.buffer.set(off, bytes[0]);
                ta.buffer.set(off + 1, bytes[1]);
            }
            TypedArrayKind::Int32 => {
                let bytes = (num as i32).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::Uint32 => {
                let bytes = (num as u32).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::Float32 => {
                let bytes = (num as f32).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::Float64 => {
                let bytes = num.to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::BigInt64 => {
                let bytes = (num as i64).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::BigUint64 => {
                let bytes = (num as u64).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
        }
        true
    }

    pub fn read_typed_array_element_vm(&mut self, ta: &JsTypedArray, index: usize) -> Option<JsValue> {
        if ta.buffer.is_detached() || index >= ta.length {
            return None;
        }
        let bpe = ta.kind.bytes_per_element();
        let off = ta.byte_offset + index * bpe;
        if off + bpe > ta.buffer.byte_length() {
            return None;
        }
        match ta.kind {
            TypedArrayKind::BigInt64 => {
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = ta.buffer.get(off + i)?;
                }
                let val = i64::from_le_bytes(bytes);
                Some(self.alloc_bigint(BigInt::from(val)))
            }
            TypedArrayKind::BigUint64 => {
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = ta.buffer.get(off + i)?;
                }
                let val = u64::from_le_bytes(bytes);
                Some(self.alloc_bigint(BigInt::from(val)))
            }
            _ => Self::read_typed_array_element(ta, index),
        }
    }

    pub fn to_bigint_op(&mut self, val: JsValue) -> Result<BigInt, VmError> {
        let prim = self.to_primitive(val, PreferredType::Number)?;
        if prim.is_nullish() {
            return Err(VmError::TypeError("Cannot convert undefined or null to a BigInt".into()));
        }
        if prim.is_symbol() {
            return Err(VmError::TypeError("Cannot convert a Symbol value to a BigInt".into()));
        }
        if prim.is_number() {
            return Err(VmError::TypeError("Cannot convert number to a BigInt".into()));
        }
        if let Some(b) = self.as_bigint_val(prim) {
            return Ok(b);
        }
        if prim.is_boolean() {
            return Ok(if prim.as_boolean() == Some(true) { BigInt::one() } else { BigInt::zero() });
        }
        let s = self.to_string_value(prim);
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Ok(BigInt::zero());
        }
        if let Some(bi) = crate::interpreter::helpers::string_to_bigint(trimmed) {
            return Ok(bi);
        }
        Err(VmError::SyntaxError("Cannot convert string to a BigInt".into()))
    }

    pub fn write_typed_array_element_vm(&mut self, ta: &JsTypedArray, index: usize, value: JsValue) -> Result<bool, VmError> {
        if ta.buffer.is_detached() || index >= ta.length {
            return Ok(false);
        }
        let bpe = ta.kind.bytes_per_element();
        let off = ta.byte_offset + index * bpe;
        if off + bpe > ta.buffer.byte_length() {
            return Ok(false);
        }
        match ta.kind {
            TypedArrayKind::BigInt64 => {
                let bi = self.to_bigint_op(value)?;
                let (sign, digits) = bi.to_u64_digits();
                let mut val_u64 = digits.first().copied().unwrap_or(0);
                if sign == num_bigint::Sign::Minus {
                    val_u64 = (!val_u64).wrapping_add(1);
                }
                let bytes = (val_u64 as i64).to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            TypedArrayKind::BigUint64 => {
                let bi = self.to_bigint_op(value)?;
                let (sign, digits) = bi.to_u64_digits();
                let mut val_u64 = digits.first().copied().unwrap_or(0);
                if sign == num_bigint::Sign::Minus {
                    val_u64 = (!val_u64).wrapping_add(1);
                }
                let bytes = val_u64.to_le_bytes();
                for (i, &b) in bytes.iter().enumerate() {
                    ta.buffer.set(off + i, b);
                }
            }
            _ => {
                if self.as_bigint_val(value).is_some() {
                    return Err(VmError::TypeError("Cannot convert BigInt to number".into()));
                }
                Self::write_typed_array_element(ta, index, value);
            }
        }
        Ok(true)
    }
    pub fn alloc_worker(&mut self, port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>) -> JsValue {
        let obj = JsObject::new_worker(ShapeRegistry::ROOT_SHAPE_ID, None, port);
        let ptr = self.allocate_js_object(obj);
        unsafe { JsValue::from_object_ptr(ptr as *mut u8) }
    }
}
