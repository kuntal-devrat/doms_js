//! Web Cryptography API (`crypto` global and `SubtleCrypto`).

use super::*;
use sha2::{Digest, Sha256, Sha384, Sha512};
use std::sync::Arc;

pub(super) fn register_crypto(vm: &mut Vm) {
    let crypto_obj = vm.alloc_object();

    // 1. crypto.randomUUID()
    let random_uuid_fn = vm.alloc_native_function("randomUUID", 0, |vm, _this, _args| {
        let mut bytes = [0u8; 16];
        if let Err(e) = getrandom::fill(&mut bytes) {
            return Err(VmError::RuntimeError(format!("crypto.randomUUID failed: {}", e)));
        }
        // RFC 4122 v4
        bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
        bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant RFC 4122

        let uuid = format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5],
            bytes[6], bytes[7],
            bytes[8], bytes[9],
            bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
        );
        Ok(vm.alloc_string(uuid))
    });
    vm.set_property(crypto_obj, "randomUUID", random_uuid_fn);
    vm.set_property_non_enumerable(crypto_obj, "randomUUID".to_string());

    // 2. crypto.getRandomValues(typedArray)
    let get_random_values_fn =
        vm.alloc_native_function("getRandomValues", 1, |_vm, _this, args| {
            let arr_val = args.first().copied().unwrap_or(JsValue::undefined());
            let ptr = match arr_val.as_object_ptr() {
                Some(p) => p,
                None => {
                    return Err(VmError::TypeError(
                        "crypto.getRandomValues: argument must be a TypedArray".into(),
                    ));
                }
            };
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            let ta = match &mut obj.kind {
                ObjectKind::TypedArray(ta) => ta,
                _ => {
                    return Err(VmError::TypeError(
                        "crypto.getRandomValues: argument must be a TypedArray".into(),
                    ));
                }
            };

            // Per W3C Web Cryptography API: Float32Array, Float64Array are not allowed
            if matches!(ta.kind, TypedArrayKind::Float32 | TypedArrayKind::Float64) {
                return Err(VmError::TypeError(
                    "crypto.getRandomValues: Float arrays are not supported".into(),
                ));
            }

            // Quota check: byte length must not exceed 65536
            let byte_len = ta.length * ta.kind.bytes_per_element();
            if byte_len > 65536 {
                return Err(VmError::RangeError(
                    "crypto.getRandomValues: QuotaExceededError - byte length exceeds 65536".into(),
                ));
            }

            let mut buf = vec![0u8; byte_len];
            if let Err(e) = getrandom::fill(&mut buf) {
                return Err(VmError::RuntimeError(format!("crypto.getRandomValues failed: {}", e)));
            }

            for (i, &b) in buf.iter().enumerate() {
                let _ = ta.buffer.set(ta.byte_offset + i, b);
            }

            Ok(arr_val)
        });
    vm.set_property(crypto_obj, "getRandomValues", get_random_values_fn);
    vm.set_property_non_enumerable(crypto_obj, "getRandomValues".to_string());

    // 3. crypto.subtle
    let subtle_obj = vm.alloc_object();

    // subtle.digest(algorithm, data)
    let digest_fn = vm.alloc_native_function("digest", 2, |vm, _this, args| {
        let algo_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let data_arg = args.get(1).copied().unwrap_or(JsValue::undefined());

        let algo_str = if algo_arg.is_js_object() {
            let name_val = vm.get_property(algo_arg, "name");
            vm.to_string_value(name_val).to_ascii_uppercase()
        } else {
            vm.to_string_value(algo_arg).to_ascii_uppercase()
        };

        let raw_bytes = extract_bytes(vm, data_arg)?;

        let hash_bytes: Vec<u8> = match algo_str.as_str() {
            "SHA-256" | "SHA256" => {
                let mut hasher = Sha256::new();
                hasher.update(&raw_bytes);
                hasher.finalize().to_vec()
            }
            "SHA-384" | "SHA384" => {
                let mut hasher = Sha384::new();
                hasher.update(&raw_bytes);
                hasher.finalize().to_vec()
            }
            "SHA-512" | "SHA512" => {
                let mut hasher = Sha512::new();
                hasher.update(&raw_bytes);
                hasher.finalize().to_vec()
            }
            _ => {
                return Err(VmError::RuntimeError(format!(
                    "crypto.subtle.digest: NotSupportedError - unsupported algorithm '{}'",
                    algo_str
                )));
            }
        };

        let sab = doms_parallel::SharedArrayBuffer::try_new(hash_bytes.len())
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        for (i, &b) in hash_bytes.iter().enumerate() {
            let _ = sab.set(i, b);
        }
        let ab = vm.alloc_array_buffer(Arc::new(sab));

        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
        vm.resolve_promise(promise, ab);
        Ok(promise)
    });
    vm.set_property(subtle_obj, "digest", digest_fn);
    vm.set_property_non_enumerable(subtle_obj, "digest".to_string());

    vm.set_property(crypto_obj, "subtle", subtle_obj);
    vm.set_property_non_enumerable(crypto_obj, "subtle".to_string());

    vm.globals.insert("crypto".to_string(), crypto_obj);
}

fn extract_bytes(_vm: &mut Vm, val: JsValue) -> Result<Vec<u8>, VmError> {
    if let Some(ptr) = val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        match &obj.kind {
            ObjectKind::ArrayBuffer(buf) | ObjectKind::SharedArrayBuffer(buf) => {
                let len = buf.byte_length();
                let mut bytes = Vec::with_capacity(len);
                for i in 0..len {
                    bytes.push(buf.get(i).unwrap_or(0));
                }
                return Ok(bytes);
            }
            ObjectKind::TypedArray(ta) => {
                let len = ta.length * ta.kind.bytes_per_element();
                let mut bytes = Vec::with_capacity(len);
                for i in 0..len {
                    bytes.push(ta.buffer.get(ta.byte_offset + i).unwrap_or(0));
                }
                return Ok(bytes);
            }
            ObjectKind::DataView { buffer, byte_offset, byte_length, .. } => {
                let mut bytes = Vec::with_capacity(*byte_length);
                for i in 0..*byte_length {
                    bytes.push(buffer.get(byte_offset + i).unwrap_or(0));
                }
                return Ok(bytes);
            }
            _ => {}
        }
    }
    Err(VmError::TypeError(
        "crypto.subtle.digest: data must be an ArrayBuffer, TypedArray, or DataView".into(),
    ))
}
