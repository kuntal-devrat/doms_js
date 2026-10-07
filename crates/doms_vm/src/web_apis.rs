//! Web Standards Host APIs (Timers, TextEncoder/Decoder, URL, Fetch, EventTarget).

use crate::interpreter::{Vm, VmError, TimerEntry};
use crate::object::{JsObject, ObjectKind, TypedArrayKind, JsTypedArray, PromiseState};
use crate::value::JsValue;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub fn register_web_apis(vm: &mut Vm) {
    register_timers(vm);
    register_encoding(vm);
    register_url(vm);
    register_events(vm);
    register_fetch(vm);
}

// ── 1. Timers & Task Queue ──

fn register_timers(vm: &mut Vm) {
    let set_timeout = vm.alloc_native_function("setTimeout", 2, |vm, _this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        let delay_ms = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(0) as u64;
        let extra_args = if args.len() > 2 { args[2..].to_vec() } else { Vec::new() };

        let id = vm.next_timer_id;
        vm.next_timer_id += 1;
        let due_time = Instant::now() + Duration::from_millis(delay_ms);

        vm.timers.insert(
            id,
            TimerEntry {
                id,
                callback,
                args: extra_args,
                due_time,
                interval: None,
            },
        );
        Ok(JsValue::from_int32(id as i32))
    });
    vm.globals.insert("setTimeout".to_string(), set_timeout);

    let clear_timeout = vm.alloc_native_function("clearTimeout", 1, |vm, _this, args| {
        let id = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if id > 0 {
            vm.timers.remove(&(id as u32));
        }
        Ok(JsValue::undefined())
    });
    vm.globals.insert("clearTimeout".to_string(), clear_timeout);

    let set_interval = vm.alloc_native_function("setInterval", 2, |vm, _this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        let delay_ms = args.get(1).and_then(|v| v.to_int32()).unwrap_or(0).max(1) as u64;
        let extra_args = if args.len() > 2 { args[2..].to_vec() } else { Vec::new() };

        let id = vm.next_timer_id;
        vm.next_timer_id += 1;
        let interval = Duration::from_millis(delay_ms);
        let due_time = Instant::now() + interval;

        vm.timers.insert(
            id,
            TimerEntry {
                id,
                callback,
                args: extra_args,
                due_time,
                interval: Some(interval),
            },
        );
        Ok(JsValue::from_int32(id as i32))
    });
    vm.globals.insert("setInterval".to_string(), set_interval);

    let clear_interval = vm.alloc_native_function("clearInterval", 1, |vm, _this, args| {
        let id = args.first().and_then(|v| v.to_int32()).unwrap_or(0);
        if id > 0 {
            vm.timers.remove(&(id as u32));
        }
        Ok(JsValue::undefined())
    });
    vm.globals.insert("clearInterval".to_string(), clear_interval);

    let queue_microtask = vm.alloc_native_function("queueMicrotask", 1, |vm, _this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        vm.enqueue_microtask(callback, Vec::new());
        Ok(JsValue::undefined())
    });
    vm.globals.insert("queueMicrotask".to_string(), queue_microtask);
}

// ── 2. TextEncoder & TextDecoder ──

fn register_encoding(vm: &mut Vm) {
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
            kind: TypedArrayKind::Uint8,
            length: bytes.len(),
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
                kind: TypedArrayKind::Uint8,
                length: bytes.len(),
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

// ── 3. URL & URLSearchParams ──

fn parse_query_string(query: &str) -> Vec<(String, String)> {
    let clean = query.strip_prefix('?').unwrap_or(query);
    if clean.is_empty() {
        return Vec::new();
    }
    clean
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|pair| {
            if let Some((k, v)) = pair.split_once('=') {
                (k.to_string(), v.to_string())
            } else {
                (pair.to_string(), String::new())
            }
        })
        .collect()
}

fn register_url(vm: &mut Vm) {
    let usp_ctor = vm.alloc_native_function("URLSearchParams", 1, |vm, _this, args| {
        let query_str = args
            .first()
            .map(|v| vm.to_string_value(*v))
            .unwrap_or_default();
        let pairs = parse_query_string(&query_str);
        let instance = vm.alloc_object();

        let ser = pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        let ser_val = vm.alloc_string(ser);
        vm.set_property(instance, "__query__", ser_val);

        let get_fn = vm.alloc_native_function("get", 1, |vm, this, args| {
            let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let q_val = vm.get_property(this, "__query__");
            let q_str = vm.to_string_value(q_val);
            let pairs = parse_query_string(&q_str);
            for (k, v) in pairs {
                if k == key {
                    return Ok(vm.alloc_string(v));
                }
            }
            Ok(JsValue::null())
        });
        vm.set_property(instance, "get", get_fn);

        let has_fn = vm.alloc_native_function("has", 1, |vm, this, args| {
            let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let q_val = vm.get_property(this, "__query__");
            let q_str = vm.to_string_value(q_val);
            let pairs = parse_query_string(&q_str);
            let has = pairs.iter().any(|(k, _)| k == &key);
            Ok(JsValue::from_boolean(has))
        });
        vm.set_property(instance, "has", has_fn);

        let set_fn = vm.alloc_native_function("set", 2, |vm, this, args| {
            let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let val = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let q_val = vm.get_property(this, "__query__");
            let q_str = vm.to_string_value(q_val);
            let mut pairs = parse_query_string(&q_str);
            let mut found = false;
            for (k, v) in &mut pairs {
                if *k == key {
                    *v = val.clone();
                    found = true;
                    break;
                }
            }
            if !found {
                pairs.push((key, val));
            }
            let new_ser = pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
            let new_ser_val = vm.alloc_string(new_ser);
            vm.set_property(this, "__query__", new_ser_val);
            Ok(JsValue::undefined())
        });
        vm.set_property(instance, "set", set_fn);

        let append_fn = vm.alloc_native_function("append", 2, |vm, this, args| {
            let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let val = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let q_val = vm.get_property(this, "__query__");
            let q_str = vm.to_string_value(q_val);
            let mut pairs = parse_query_string(&q_str);
            pairs.push((key, val));
            let new_ser = pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
            let new_ser_val = vm.alloc_string(new_ser);
            vm.set_property(this, "__query__", new_ser_val);
            Ok(JsValue::undefined())
        });
        vm.set_property(instance, "append", append_fn);

        let delete_fn = vm.alloc_native_function("delete", 1, |vm, this, args| {
            let key = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let q_val = vm.get_property(this, "__query__");
            let q_str = vm.to_string_value(q_val);
            let mut pairs = parse_query_string(&q_str);
            pairs.retain(|(k, _)| k != &key);
            let new_ser = pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
            let new_ser_val = vm.alloc_string(new_ser);
            vm.set_property(this, "__query__", new_ser_val);
            Ok(JsValue::undefined())
        });
        vm.set_property(instance, "delete", delete_fn);

        let to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let q_val = vm.get_property(this, "__query__");
            Ok(q_val)
        });
        vm.set_property(instance, "toString", to_string_fn);

        Ok(instance)
    });
    vm.globals.insert("URLSearchParams".to_string(), usp_ctor);

    // URL constructor
    let url_ctor = vm.alloc_native_function("URL", 2, |vm, _this, args| {
        let input = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let base = args.get(1).map(|v| vm.to_string_value(*v));

        let full_url = if let Some(base_str) = base {
            if input.starts_with("http://") || input.starts_with("https://") {
                input
            } else if input.starts_with('/') {
                let base_root = if let Some(slash_idx) = base_str[8..].find('/') {
                    &base_str[..8 + slash_idx]
                } else {
                    &base_str
                };
                format!("{}{}", base_root, input)
            } else {
                let trimmed_base = base_str.trim_end_matches('/');
                format!("{}/{}", trimmed_base, input)
            }
        } else {
            input
        };

        let (protocol, rest) = if let Some((proto, r)) = full_url.split_once("://") {
            (format!("{}:", proto), r)
        } else {
            ("http:".to_string(), full_url.as_str())
        };

        let (host_part, path_and_query) = if let Some((h, pq)) = rest.split_once('/') {
            (h.to_string(), format!("/{}", pq))
        } else {
            (rest.to_string(), "/".to_string())
        };

        let (hostname, port) = if let Some((h, p)) = host_part.split_once(':') {
            (h.to_string(), p.to_string())
        } else {
            (host_part.clone(), String::new())
        };

        let (pathname, search_and_hash) = if let Some((p, sh)) = path_and_query.split_once('?') {
            (p.to_string(), format!("?{}", sh))
        } else {
            (path_and_query.clone(), String::new())
        };

        let (search, hash) = if let Some((s, h)) = search_and_hash.split_once('#') {
            (s.to_string(), format!("#{}", h))
        } else {
            (search_and_hash, String::new())
        };

        let origin = format!("{}//{}", protocol, host_part);

        let instance = vm.alloc_object();
        let href_val = vm.alloc_string(full_url);
        let proto_val = vm.alloc_string(protocol);
        let host_val = vm.alloc_string(host_part);
        let hostname_val = vm.alloc_string(hostname);
        let port_val = vm.alloc_string(port);
        let path_val = vm.alloc_string(pathname);
        let search_val = vm.alloc_string(search.clone());
        let hash_val = vm.alloc_string(hash);
        let origin_val = vm.alloc_string(origin);

        vm.set_property(instance, "href", href_val);
        vm.set_property(instance, "protocol", proto_val);
        vm.set_property(instance, "host", host_val);
        vm.set_property(instance, "hostname", hostname_val);
        vm.set_property(instance, "port", port_val);
        vm.set_property(instance, "pathname", path_val);
        vm.set_property(instance, "search", search_val);
        vm.set_property(instance, "hash", hash_val);
        vm.set_property(instance, "origin", origin_val);

        // searchParams instance
        let usp_ctor = vm.globals.get("URLSearchParams").copied().unwrap_or(JsValue::undefined());
        let usp_init = vm.alloc_string(search);
        let usp_obj = vm.call(usp_ctor, JsValue::undefined(), &[usp_init])?;
        vm.set_property(instance, "searchParams", usp_obj);

        let to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let h = vm.get_property(this, "href");
            Ok(h)
        });
        vm.set_property(instance, "toString", to_string_fn);

        Ok(instance)
    });
    vm.globals.insert("URL".to_string(), url_ctor);
}

// ── 4. EventTarget, Event & CustomEvent ──

fn register_events(vm: &mut Vm) {
    let event_ctor = vm.alloc_native_function("Event", 2, |vm, _this, args| {
        let type_str = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let options = args.get(1).copied().unwrap_or(JsValue::undefined());

        let instance = vm.alloc_object();
        let type_val = vm.alloc_string(type_str);
        vm.set_property(instance, "type", type_val);
        vm.set_property(instance, "target", JsValue::null());

        let mut bubbles = false;
        let mut cancelable = false;
        if options.is_object() {
            let b = vm.get_property(options, "bubbles");
            let c = vm.get_property(options, "cancelable");
            bubbles = b.is_truthy();
            cancelable = c.is_truthy();
        }
        vm.set_property(instance, "bubbles", JsValue::from_boolean(bubbles));
        vm.set_property(instance, "cancelable", JsValue::from_boolean(cancelable));
        vm.set_property(instance, "defaultPrevented", JsValue::from_boolean(false));

        let prev_fn = vm.alloc_native_function("preventDefault", 0, |vm, this, _args| {
            let cancelable = vm.get_property(this, "cancelable").is_truthy();
            if cancelable {
                vm.set_property(this, "defaultPrevented", JsValue::from_boolean(true));
            }
            Ok(JsValue::undefined())
        });
        vm.set_property(instance, "preventDefault", prev_fn);

        Ok(instance)
    });
    vm.globals.insert("Event".to_string(), event_ctor);

    let custom_event_ctor = vm.alloc_native_function("CustomEvent", 2, |vm, _this, args| {
        let event_ctor = vm.globals.get("Event").copied().unwrap_or(JsValue::undefined());
        let type_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let options = args.get(1).copied().unwrap_or(JsValue::undefined());

        let event_obj = vm.call(event_ctor, JsValue::undefined(), &[type_arg, options])?;
        let detail = if options.is_object() {
            vm.get_property(options, "detail")
        } else {
            JsValue::null()
        };
        vm.set_property(event_obj, "detail", detail);
        Ok(event_obj)
    });
    vm.globals.insert("CustomEvent".to_string(), custom_event_ctor);

    let event_target_ctor = vm.alloc_native_function("EventTarget", 0, |vm, _this, _args| {
        let instance = vm.alloc_object();
        let listeners_obj = vm.alloc_object();
        vm.set_property(instance, "__listeners__", listeners_obj);

        let add_fn = vm.alloc_native_function("addEventListener", 2, |vm, this, args| {
            let event_type = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let listener = args.get(1).copied().unwrap_or(JsValue::undefined());
            let listeners = vm.get_property(this, "__listeners__");
            let arr_val = vm.get_property(listeners, &event_type);
            let arr = if arr_val.is_object() {
                arr_val
            } else {
                let new_arr = vm.alloc_array(Vec::new());
                vm.set_property(listeners, &event_type, new_arr);
                new_arr
            };
            if let Some(ptr) = arr.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &mut obj.kind {
                    elements.push(listener);
                }
            }
            Ok(JsValue::undefined())
        });
        vm.set_property(instance, "addEventListener", add_fn);

        let remove_fn = vm.alloc_native_function("removeEventListener", 2, |vm, this, args| {
            let event_type = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
            let listener = args.get(1).copied().unwrap_or(JsValue::undefined());
            let listeners = vm.get_property(this, "__listeners__");
            let arr_val = vm.get_property(listeners, &event_type);
            if let Some(ptr) = arr_val.as_object_ptr() {
                let obj = unsafe { &mut *(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &mut obj.kind {
                    elements.retain(|&e| e.raw_bits() != listener.raw_bits());
                }
            }
            Ok(JsValue::undefined())
        });
        vm.set_property(instance, "removeEventListener", remove_fn);

        let dispatch_fn = vm.alloc_native_function("dispatchEvent", 1, |vm, this, args| {
            let event = args.first().copied().unwrap_or(JsValue::undefined());
            if !event.is_object() {
                return Err(VmError::TypeError("Event must be an object".into()));
            }
            vm.set_property(event, "target", this);
            let type_val = vm.get_property(event, "type");
            let event_type = vm.to_string_value(type_val);
            let listeners = vm.get_property(this, "__listeners__");
            let arr_val = vm.get_property(listeners, &event_type);

            let mut callbacks = Vec::new();
            if let Some(ptr) = arr_val.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::Array(elements) = &obj.kind {
                    callbacks = elements.clone();
                }
            }
            for cb in callbacks {
                let _ = vm.call(cb, this, &[event]);
            }
            let def_prevented = vm.get_property(event, "defaultPrevented").is_truthy();
            Ok(JsValue::from_boolean(!def_prevented))
        });
        vm.set_property(instance, "dispatchEvent", dispatch_fn);

        Ok(instance)
    });
    vm.globals.insert("EventTarget".to_string(), event_target_ctor);
}

// ── 5. Fetch API & Headers / Response ──

fn register_fetch(vm: &mut Vm) {
    let headers_ctor = vm.alloc_native_function("Headers", 1, |vm, _this, args| {
        let instance = vm.alloc_object();
        let store = vm.alloc_object();
        vm.set_property(instance, "__store__", store);

        let init = args.first().copied().unwrap_or(JsValue::undefined());
        if init.is_object() {
            if let Some(ptr) = init.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let Some(shape) = vm.shapes.get(obj.shape_id) {
                    let pairs: Vec<(String, JsValue)> = shape
                        .property_offsets
                        .iter()
                        .filter_map(|(k, info)| obj.get_slot(info.offset).map(|v| (k.clone(), v)))
                        .collect();
                    for (k, v) in pairs {
                        vm.set_property(store, &k.to_ascii_lowercase(), v);
                    }
                }
            }
        }

        let get_fn = vm.alloc_native_function("get", 1, |vm, this, args| {
            let name = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default().to_ascii_lowercase();
            let store = vm.get_property(this, "__store__");
            let val = vm.get_property(store, &name);
            if val.is_undefined() {
                Ok(JsValue::null())
            } else {
                Ok(val)
            }
        });
        vm.set_property(instance, "get", get_fn);

        let set_fn = vm.alloc_native_function("set", 2, |vm, this, args| {
            let name = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default().to_ascii_lowercase();
            let val = args.get(1).copied().unwrap_or(JsValue::undefined());
            let store = vm.get_property(this, "__store__");
            vm.set_property(store, &name, val);
            Ok(JsValue::undefined())
        });
        vm.set_property(instance, "set", set_fn);

        let has_fn = vm.alloc_native_function("has", 1, |vm, this, args| {
            let name = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default().to_ascii_lowercase();
            let store = vm.get_property(this, "__store__");
            let val = vm.get_property(store, &name);
            Ok(JsValue::from_boolean(!val.is_undefined()))
        });
        vm.set_property(instance, "has", has_fn);

        Ok(instance)
    });
    vm.globals.insert("Headers".to_string(), headers_ctor);

    let response_ctor = vm.alloc_native_function("Response", 2, |vm, _this, args| {
        let body_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let init = args.get(1).copied().unwrap_or(JsValue::undefined());

        let instance = vm.alloc_object();

        let mut status = 200;
        let mut status_text = "OK".to_string();
        let headers_ctor = vm.globals.get("Headers").copied().unwrap_or(JsValue::undefined());
        let headers_obj = vm.call(headers_ctor, JsValue::undefined(), &[])?;

        if init.is_object() {
            let st = vm.get_property(init, "status");
            if let Some(num) = st.to_int32() {
                status = num;
            }
            let st_text = vm.get_property(init, "statusText");
            if !st_text.is_undefined() {
                status_text = vm.to_string_value(st_text);
            }
            let h_init = vm.get_property(init, "headers");
            if h_init.is_object() {
                if let Some(ptr) = h_init.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    if let Some(shape) = vm.shapes.get(obj.shape_id) {
                        let pairs: Vec<(String, JsValue)> = shape
                            .property_offsets
                            .iter()
                            .filter_map(|(k, info)| obj.get_slot(info.offset).map(|v| (k.clone(), v)))
                            .collect();
                        let store = vm.get_property(headers_obj, "__store__");
                        for (k, v) in pairs {
                            vm.set_property(store, &k.to_ascii_lowercase(), v);
                        }
                    }
                }
            }
        }

        let body_str = if body_arg.is_undefined() {
            String::new()
        } else {
            vm.to_string_value(body_arg)
        };

        let body_val = vm.alloc_string(body_str);
        vm.set_property(instance, "__body__", body_val);
        vm.set_property(instance, "status", JsValue::from_int32(status));
        let st_text_val = vm.alloc_string(status_text);
        vm.set_property(instance, "statusText", st_text_val);
        let ok = (200..300).contains(&status);
        vm.set_property(instance, "ok", JsValue::from_boolean(ok));
        vm.set_property(instance, "headers", headers_obj);

        let text_fn = vm.alloc_native_function("text", 0, |vm, this, _args| {
            let b = vm.get_property(this, "__body__");
            let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            vm.resolve_promise(promise, b);
            Ok(promise)
        });
        vm.set_property(instance, "text", text_fn);

        let json_fn = vm.alloc_native_function("json", 0, |vm, this, _args| {
            let b = vm.get_property(this, "__body__");
            let s = vm.to_string_value(b);
            let json_global = vm.globals.get("JSON").copied().unwrap_or(JsValue::undefined());
            let parse_fn = vm.get_property(json_global, "parse");
            let s_val = vm.alloc_string(s);
            let parse_res = vm.call(parse_fn, json_global, &[s_val])?;
            let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            vm.resolve_promise(promise, parse_res);
            Ok(promise)
        });
        vm.set_property(instance, "json", json_fn);

        let ab_fn = vm.alloc_native_function("arrayBuffer", 0, |vm, this, _args| {
            let b = vm.get_property(this, "__body__");
            let s = vm.to_string_value(b);
            let bytes = s.as_bytes();
            let sab = doms_parallel::SharedArrayBuffer::try_new(bytes.len())
                .map_err(|e| VmError::RuntimeError(e.into()))?;
            for (i, &byte) in bytes.iter().enumerate() {
                let _ = sab.set(i, byte);
            }
            let sab_val = vm.alloc_shared_array_buffer(Arc::new(sab));
            let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            vm.resolve_promise(promise, sab_val);
            Ok(promise)
        });
        vm.set_property(instance, "arrayBuffer", ab_fn);

        Ok(instance)
    });
    vm.globals.insert("Response".to_string(), response_ctor);

    let fetch_fn = vm.alloc_native_function("fetch", 2, |vm, _this, args| {
        let input = args.first().copied().unwrap_or(JsValue::undefined());
        let _init = args.get(1).copied().unwrap_or(JsValue::undefined());
        let url_str = vm.to_string_value(input);

        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

        let (status, status_text, body) = if url_str.starts_with("http://") {
            perform_simple_http_get(&url_str)
        } else if let Some(stripped) = url_str.strip_prefix("data:") {
            let content = if let Some((_, b)) = stripped.split_once(',') {
                b.to_string()
            } else {
                stripped.to_string()
            };
            (200, "OK".to_string(), content)
        } else {
            (200, "OK".to_string(), format!("{{\"url\":\"{}\",\"status\":\"ok\"}}", url_str))
        };

        let body_val = vm.alloc_string(body);
        let resp_init = vm.alloc_object();
        vm.set_property(resp_init, "status", JsValue::from_int32(status));
        let st_text = vm.alloc_string(status_text);
        vm.set_property(resp_init, "statusText", st_text);

        let response_ctor = vm.globals.get("Response").copied().unwrap_or(JsValue::undefined());
        let resp_obj = vm.call(response_ctor, JsValue::undefined(), &[body_val, resp_init])?;
        let url_val = vm.alloc_string(url_str);
        vm.set_property(resp_obj, "url", url_val);

        vm.resolve_promise(promise, resp_obj);
        Ok(promise)
    });
    vm.globals.insert("fetch".to_string(), fetch_fn);
}

fn perform_simple_http_get(url_str: &str) -> (i32, String, String) {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let stripped = url_str.strip_prefix("http://").unwrap_or(url_str);
    let (host_port, path) = if let Some((h, p)) = stripped.split_once('/') {
        (h, format!("/{}", p))
    } else {
        (stripped, "/".to_string())
    };

    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (h, p.parse::<u16>().unwrap_or(80))
    } else {
        (host_port, 80)
    };

    let addr = format!("{}:{}", host, port);
    let mut stream = match TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| "127.0.0.1:80".parse().unwrap()),
        Duration::from_millis(500),
    ) {
        Ok(s) => s,
        Err(_) => return (502, "Bad Gateway".to_string(), "Connection failed".to_string()),
    };

    let _ = stream.set_read_timeout(Some(Duration::from_millis(1000)));
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: DOMS/0.1.0\r\nConnection: close\r\n\r\n",
        path, host
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return (502, "Bad Gateway".to_string(), "Failed to send request".to_string());
    }

    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let resp_str = String::from_utf8_lossy(&response);

    if let Some((header, body)) = resp_str.split_once("\r\n\r\n") {
        let status_line = header.lines().next().unwrap_or("HTTP/1.1 200 OK");
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        let code = parts.get(1).and_then(|c| c.parse::<i32>().ok()).unwrap_or(200);
        let status_text = parts.get(2..).map(|p| p.join(" ")).unwrap_or_else(|| "OK".to_string());
        (code, status_text, body.to_string())
    } else {
        (200, "OK".to_string(), resp_str.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timers_and_event_loop() {
        let mut vm = Vm::new();
        let script = r#"
            var timerFired = false;
            setTimeout(function() {
                timerFired = true;
            }, 5);
        "#;
        vm.eval(script).expect("eval should succeed");
        assert_eq!(vm.globals.get("timerFired").map(|v| v.is_truthy()), Some(false));

        // Sleep 15ms so timer expires
        std::thread::sleep(Duration::from_millis(15));
        vm.run_event_loop().expect("event loop should run");
        assert_eq!(vm.globals.get("timerFired").map(|v| v.is_truthy()), Some(true));
    }

    #[test]
    fn test_text_encoder_decoder() {
        let mut vm = Vm::new();
        let script = r#"
            var encoder = new TextEncoder();
            var bytes = encoder.encode("Hello, DOMS!");
            var decoder = new TextDecoder();
            var result = decoder.decode(bytes);
        "#;
        vm.eval(script).expect("eval should succeed");
        let res = vm.globals.get("result").copied().unwrap();
        assert_eq!(vm.to_string_value(res), "Hello, DOMS!");
    }

    #[test]
    fn test_url_and_search_params() {
        let mut vm = Vm::new();
        let script = r#"
            var url = new URL("https://domsjs.dev:8080/api/v1/search?query=parallel&page=2#top");
            var protocol = url.protocol;
            var hostname = url.hostname;
            var port = url.port;
            var pathname = url.pathname;
            var query = url.searchParams.get("query");
            var hasPage = url.searchParams.has("page");
        "#;
        vm.eval(script).expect("eval should succeed");
        assert_eq!(vm.to_string_value(vm.globals.get("protocol").copied().unwrap()), "https:");
        assert_eq!(vm.to_string_value(vm.globals.get("hostname").copied().unwrap()), "domsjs.dev");
        assert_eq!(vm.to_string_value(vm.globals.get("port").copied().unwrap()), "8080");
        assert_eq!(vm.to_string_value(vm.globals.get("pathname").copied().unwrap()), "/api/v1/search");
        assert_eq!(vm.to_string_value(vm.globals.get("query").copied().unwrap()), "parallel");
        assert_eq!(vm.globals.get("hasPage").map(|v| v.is_truthy()), Some(true));
    }

    #[test]
    fn test_event_target_and_custom_event() {
        let mut vm = Vm::new();
        let script = r#"
            var target = new EventTarget();
            var receivedDetail = null;
            target.addEventListener("custom_event", function(e) {
                receivedDetail = e.detail;
            });
            var event = new CustomEvent("custom_event", { detail: "DOMS_EVENT_PAYLOAD" });
            target.dispatchEvent(event);
        "#;
        vm.eval(script).expect("eval should succeed");
        let detail = vm.globals.get("receivedDetail").copied().unwrap();
        assert_eq!(vm.to_string_value(detail), "DOMS_EVENT_PAYLOAD");
    }

    #[test]
    fn test_fetch_response_mock() {
        let mut vm = Vm::new();
        let script = r#"
            var fetchBody = null;
            fetch("data:text/plain,Hello DOMS Fetch").then(function(res) {
                return res.text();
            }).then(function(text) {
                fetchBody = text;
            });
        "#;
        vm.eval(script).expect("eval should succeed");
        vm.drain_microtasks().expect("drain microtasks");
        let body = vm.globals.get("fetchBody").copied().unwrap();
        assert_eq!(vm.to_string_value(body), "Hello DOMS Fetch");
    }
}
