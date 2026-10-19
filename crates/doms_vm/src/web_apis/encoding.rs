//! `TextEncoder` and `TextDecoder`.

use super::*;

pub(super) fn register_encoding(vm: &mut Vm) {
    let text_encoder_proto = vm.alloc_object();
    let encoding_val = vm.alloc_string("utf-8".to_string());
    vm.set_property(text_encoder_proto, "encoding", encoding_val);

    let encode_fn = vm.alloc_native_function("encode", 1, |vm, _this, args| {
        let text = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let bytes = text.as_bytes();
        let sab = doms_parallel::SharedArrayBuffer::try_new(bytes.len())
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        for (i, &b) in bytes.iter().enumerate() {
            let _ = sab.set(i, b);
        }
        Ok(vm.alloc_typed_array(JsTypedArray {
            buffer: Arc::new(sab),
            buffer_val: None,
            kind: TypedArrayKind::Uint8,
            length: bytes.len(),
            byte_offset: 0,
        }))
    });
    vm.set_property(text_encoder_proto, "encode", encode_fn);

    let text_encoder_ctor = vm.alloc_native_function("TextEncoder", 0, |vm, _this, _args| {
        let instance = vm.alloc_object();
        let enc_str = vm.alloc_string("utf-8".to_string());
        vm.set_property(instance, "encoding", enc_str);
        let encode_fn = vm.alloc_native_function("encode", 1, |vm, _this, args| {
            let text = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let bytes = text.as_bytes();
            let sab = doms_parallel::SharedArrayBuffer::try_new(bytes.len())
                .map_err(|e| VmError::RuntimeError(e.into()))?;
            for (i, &b) in bytes.iter().enumerate() {
                let _ = sab.set(i, b);
            }
            Ok(vm.alloc_typed_array(JsTypedArray {
                buffer: Arc::new(sab),
                buffer_val: None,
                kind: TypedArrayKind::Uint8,
                length: bytes.len(),
                byte_offset: 0,
            }))
        });
        vm.set_property(instance, "encode", encode_fn);
        Ok(instance)
    });
    vm.set_property(text_encoder_ctor, "prototype", text_encoder_proto);
    vm.globals.insert("TextEncoder".to_string(), text_encoder_ctor);

    // TextDecoder
    let text_decoder_proto = vm.alloc_object();
    vm.set_property(text_decoder_proto, "encoding", encoding_val);

    let decode_fn = vm.alloc_native_function("decode", 1, native_decode_fn);
    vm.set_property(text_decoder_proto, "decode", decode_fn);

    let text_decoder_ctor = vm.alloc_native_function("TextDecoder", 1, |vm, _this, _args| {
        let instance = vm.alloc_object();
        let enc_str = vm.alloc_string("utf-8".to_string());
        vm.set_property(instance, "encoding", enc_str);
        let decode_fn = vm.alloc_native_function("decode", 1, native_decode_fn);
        vm.set_property(instance, "decode", decode_fn);
        Ok(instance)
    });
    vm.set_property(text_decoder_ctor, "prototype", text_decoder_proto);
    vm.globals.insert("TextDecoder".to_string(), text_decoder_ctor);
}

fn native_decode_fn(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let input = args.first().copied().unwrap_or(JsValue::undefined());
    if let Some(ptr) = input.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::TypedArray(ta) = &obj.kind {
            let mut bytes = Vec::with_capacity(ta.length);
            for i in 0..ta.length {
                bytes.push(ta.buffer.get(i).unwrap_or(0));
            }
            let decoded = String::from_utf8_lossy(&bytes).into_owned();
            return Ok(vm.alloc_string(decoded));
        }
    }
    if input.is_undefined() {
        return Ok(vm.alloc_string("".to_string()));
    }
    let s = vm.to_string_value(input);
    Ok(vm.alloc_string(s))
}
