//! WHATWG Fetch API (`fetch`, `Request`, `Response`, `Headers`, `AbortController`, `AbortSignal`).

use super::*;
use std::io::Read;

pub(super) fn register_fetch(vm: &mut Vm) {
    register_abort_api(vm);
    register_headers(vm);
    register_request(vm);
    register_response(vm);
    register_fetch_fn(vm);
}

fn register_abort_api(vm: &mut Vm) {
    let abort_signal_proto = vm.alloc_object();

    let aborted_getter = vm.alloc_native_function("get aborted", 0, |vm, this, _args| {
        let val = vm.get_property(this, "__aborted__");
        Ok(JsValue::from_boolean(val.is_truthy()))
    });
    let reason_getter = vm.alloc_native_function("get reason", 0, |vm, this, _args| {
        let val = vm.get_property(this, "__reason__");
        Ok(val)
    });
    let throw_if_aborted = vm.alloc_native_function("throwIfAborted", 0, |vm, this, _args| {
        let aborted = vm.get_property(this, "__aborted__");
        if aborted.is_truthy() {
            let reason = vm.get_property(this, "__reason__");
            return Err(VmError::Thrown(reason));
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(abort_signal_proto, "throwIfAborted", throw_if_aborted);

    if let Some(ptr) = abort_signal_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor("aborted".to_string(), Some(aborted_getter), None);
        obj.define_builtin_accessor("reason".to_string(), Some(reason_getter), None);
    }

    // Set EventTarget as prototype of AbortSignal prototype
    if let Some(et_ctor) = vm.globals.get("EventTarget").copied() {
        let et_proto = vm.get_property(et_ctor, "prototype");
        if et_proto.is_js_object() {
            vm.set_prototype(abort_signal_proto, et_proto);
        }
    }

    let abort_signal_ctor = vm.alloc_native_function("AbortSignal", 0, |_vm, _this, _args| {
        Err(VmError::TypeError(
            "Illegal constructor: use AbortController or AbortSignal.abort()".into(),
        ))
    });
    vm.set_property(abort_signal_ctor, "prototype", abort_signal_proto);

    // AbortSignal.abort(reason)
    let static_abort = vm.alloc_native_function("abort", 1, |vm, _this, args| {
        let reason = args.first().copied().unwrap_or_else(|| {
            let err_ctor = vm.globals.get("Error").copied().unwrap_or(JsValue::undefined());
            let msg = vm.alloc_string("This operation was aborted".into());
            vm.call(err_ctor, JsValue::undefined(), &[msg]).unwrap_or(msg)
        });

        let ctor = vm.globals.get("AbortSignal").copied().unwrap_or(JsValue::undefined());
        let proto = vm.get_property(ctor, "prototype");
        let sig = vm.alloc_object();
        vm.set_prototype(sig, proto);
        vm.set_property(sig, "__aborted__", JsValue::from_boolean(true));
        vm.set_property(sig, "__reason__", reason);
        Ok(sig)
    });
    vm.set_property(abort_signal_ctor, "abort", static_abort);

    // AbortSignal.timeout(ms)
    let static_timeout = vm.alloc_native_function("timeout", 1, |vm, _this, args| {
        let ms = args.first().copied().unwrap_or(JsValue::from_int32(0));
        let timeout_script = r#"
            (function(ms) {
                var controller = new AbortController();
                setTimeout(function() {
                    var err = new Error("The operation timed out");
                    err.name = "TimeoutError";
                    controller.abort(err);
                }, ms);
                return controller.signal;
            })
        "#;
        let helper = vm.eval(timeout_script)?;
        vm.call(helper, JsValue::undefined(), &[ms])
    });
    vm.set_property(abort_signal_ctor, "timeout", static_timeout);
    vm.globals.insert("AbortSignal".to_string(), abort_signal_ctor);

    // AbortController
    let abort_controller_proto = vm.alloc_object();
    let abort_fn = vm.alloc_native_function("abort", 1, |vm, this, args| {
        let sig = vm.get_property(this, "signal");
        let already_aborted = vm.get_property(sig, "__aborted__");
        if !already_aborted.is_truthy() {
            let reason = args.first().copied().unwrap_or_else(|| {
                let err_ctor = vm.globals.get("Error").copied().unwrap_or(JsValue::undefined());
                let msg = vm.alloc_string("This operation was aborted".into());
                vm.call(err_ctor, JsValue::undefined(), &[msg]).unwrap_or(msg)
            });
            vm.set_property(sig, "__aborted__", JsValue::from_boolean(true));
            vm.set_property(sig, "__reason__", reason);

            // Dispatch event
            let disp_fn = vm.get_property(sig, "dispatchEvent");
            if vm.is_callable(disp_fn) {
                let ev_ctor = vm.globals.get("Event").copied().unwrap_or(JsValue::undefined());
                let ev_type = vm.alloc_string("abort".into());
                if let Ok(ev) = vm.call(ev_ctor, JsValue::undefined(), &[ev_type]) {
                    let _ = vm.call(disp_fn, sig, &[ev]);
                }
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(abort_controller_proto, "abort", abort_fn);

    let abort_controller_ctor =
        vm.alloc_native_function("AbortController", 0, |vm, _this, _args| {
            let ctor = vm.globals.get("AbortController").copied().unwrap_or(JsValue::undefined());
            let proto = vm.get_property(ctor, "prototype");
            let instance = vm.alloc_object();
            vm.set_prototype(instance, proto);

            let sig_ctor = vm.globals.get("AbortSignal").copied().unwrap_or(JsValue::undefined());
            let sig_proto = vm.get_property(sig_ctor, "prototype");
            let sig = vm.alloc_object();
            vm.set_prototype(sig, sig_proto);
            vm.set_property(sig, "__aborted__", JsValue::from_boolean(false));
            vm.set_property(sig, "__reason__", JsValue::undefined());

            vm.set_property(instance, "signal", sig);
            Ok(instance)
        });
    vm.set_property(abort_controller_ctor, "prototype", abort_controller_proto);
    vm.globals.insert("AbortController".to_string(), abort_controller_ctor);
}

fn register_headers(vm: &mut Vm) {
    let headers_proto = vm.alloc_object();

    let get_fn = vm.alloc_native_function("get", 1, |vm, this, args| {
        let name =
            args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default().to_ascii_lowercase();
        let store = vm.get_property(this, "__store__");
        let val = vm.get_property(store, &name);
        if val.is_undefined() { Ok(JsValue::null()) } else { Ok(val) }
    });
    vm.set_property(headers_proto, "get", get_fn);

    let set_fn = vm.alloc_native_function("set", 2, |vm, this, args| {
        let name =
            args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default().to_ascii_lowercase();
        let val = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let val_val = vm.alloc_string(val);
        let store = vm.get_property(this, "__store__");
        vm.set_property(store, &name, val_val);
        Ok(JsValue::undefined())
    });
    vm.set_property(headers_proto, "set", set_fn);

    let append_fn = vm.alloc_native_function("append", 2, |vm, this, args| {
        let name =
            args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default().to_ascii_lowercase();
        let val = args.get(1).map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let store = vm.get_property(this, "__store__");
        let existing = vm.get_property(store, &name);
        let new_val = if existing.is_undefined() {
            val
        } else {
            format!("{}, {}", vm.to_string_value(existing), val)
        };
        let val_val = vm.alloc_string(new_val);
        vm.set_property(store, &name, val_val);
        Ok(JsValue::undefined())
    });
    vm.set_property(headers_proto, "append", append_fn);

    let has_fn = vm.alloc_native_function("has", 1, |vm, this, args| {
        let name =
            args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default().to_ascii_lowercase();
        let store = vm.get_property(this, "__store__");
        let val = vm.get_property(store, &name);
        Ok(JsValue::from_boolean(!val.is_undefined()))
    });
    vm.set_property(headers_proto, "has", has_fn);

    let delete_fn = vm.alloc_native_function("delete", 1, |vm, this, args| {
        let name =
            args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default().to_ascii_lowercase();
        let store = vm.get_property(this, "__store__");
        let _ = vm.delete_property(store, &name);
        Ok(JsValue::undefined())
    });
    vm.set_property(headers_proto, "delete", delete_fn);

    let for_each_fn = vm.alloc_native_function("forEach", 2, |vm, this, args| {
        let callback = args.first().copied().unwrap_or(JsValue::undefined());
        let this_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        if !vm.is_callable(callback) {
            return Err(VmError::TypeError("Headers.forEach callback must be callable".into()));
        }
        let store = vm.get_property(this, "__store__");
        if let Some(ptr) = store.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let entries: Vec<(String, JsValue)> = vm
                .shapes
                .all_property_entries(obj.shape_id)
                .into_iter()
                .filter_map(|(k, offset)| obj.get_slot(offset).map(|v| (k, v)))
                .collect();
            for (k, v) in entries {
                let k_val = vm.alloc_string(k);
                let _ = vm.call(callback, this_arg, &[v, k_val, this]);
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(headers_proto, "forEach", for_each_fn);

    let entries_fn = vm.alloc_native_function("entries", 0, |vm, this, _args| {
        let store = vm.get_property(this, "__store__");
        let array = vm.alloc_array(Vec::new());
        if let Some(ptr) = store.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            let entries: Vec<(String, JsValue)> = vm
                .shapes
                .all_property_entries(obj.shape_id)
                .into_iter()
                .filter_map(|(k, offset)| obj.get_slot(offset).map(|v| (k, v)))
                .collect();
            let push_fn = vm.get_property(array, "push");
            for (k, v) in entries {
                let k_val = vm.alloc_string(k);
                let pair = vm.alloc_array(vec![k_val, v]);
                let _ = vm.call(push_fn, array, &[pair]);
            }
        }
        let sym_iter = vm.symbol_iterator;
        if let Some(sym) = sym_iter {
            let iter_fn = vm.get_symbol_property(array, sym);
            if vm.is_callable(iter_fn) {
                return vm.call(iter_fn, array, &[]);
            }
        }
        Ok(array)
    });
    vm.set_property(headers_proto, "entries", entries_fn);
    if let Some(sym_iter) = vm.symbol_iterator {
        vm.set_symbol_property(headers_proto, sym_iter, entries_fn);
    }

    let headers_ctor = vm.alloc_native_function("Headers", 1, |vm, _this, args| {
        let ctor = vm.globals.get("Headers").copied().unwrap_or(JsValue::undefined());
        let proto = vm.get_property(ctor, "prototype");
        let instance = vm.alloc_object();
        vm.set_prototype(instance, proto);
        let store = vm.alloc_object();
        vm.set_property(instance, "__store__", store);

        let init = args.first().copied().unwrap_or(JsValue::undefined());
        if init.is_js_object() {
            let init_script = r#"
                (function(headers, init) {
                    if (init.forEach) {
                        init.forEach(function(v, k) {
                            headers.set(k, v);
                        });
                    }
                })
            "#;
            if let Ok(helper) = vm.eval(init_script) {
                let _ = vm.call(helper, JsValue::undefined(), &[instance, init]);
            }

            if let Some(ptr) = init.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                let pairs: Vec<(String, JsValue)> = vm
                    .shapes
                    .all_property_entries(obj.shape_id)
                    .into_iter()
                    .filter_map(|(k, offset)| obj.get_slot(offset).map(|v| (k, v)))
                    .collect();
                for (k, v) in pairs {
                    let val_str = vm.to_string_value(v);
                    let val_val = vm.alloc_string(val_str);
                    vm.set_property(store, &k.to_ascii_lowercase(), val_val);
                }
            }
        }

        Ok(instance)
    });
    vm.set_property(headers_ctor, "prototype", headers_proto);
    vm.globals.insert("Headers".to_string(), headers_ctor);
}

fn register_request(vm: &mut Vm) {
    let request_proto = vm.alloc_object();

    let request_ctor = vm.alloc_native_function("Request", 2, |vm, _this, args| {
        let input = args.first().copied().unwrap_or(JsValue::undefined());
        let init = args.get(1).copied().unwrap_or(JsValue::undefined());

        let ctor = vm.globals.get("Request").copied().unwrap_or(JsValue::undefined());
        let proto = vm.get_property(ctor, "prototype");
        let instance = vm.alloc_object();
        vm.set_prototype(instance, proto);

        let url_str = if input.is_js_object() {
            let u = vm.get_property(input, "url");
            vm.to_string_value(u)
        } else {
            vm.to_string_value(input)
        };
        let mut method_str = "GET".to_string();
        let mut signal_val = JsValue::undefined();

        if input.is_js_object() {
            let m = vm.get_property(input, "method");
            if !m.is_undefined() {
                method_str = vm.to_string_value(m).to_ascii_uppercase();
            }
            let s = vm.get_property(input, "signal");
            if !s.is_undefined() {
                signal_val = s;
            }
        }

        let headers_ctor = vm.globals.get("Headers").copied().unwrap_or(JsValue::undefined());
        let mut headers_obj = vm.call(headers_ctor, JsValue::undefined(), &[])?;

        if init.is_js_object() {
            let m = vm.get_property(init, "method");
            if !m.is_undefined() {
                method_str = vm.to_string_value(m).to_ascii_uppercase();
            }
            let h = vm.get_property(init, "headers");
            if !h.is_undefined() {
                headers_obj = vm.call(headers_ctor, JsValue::undefined(), &[h])?;
            }
            let s = vm.get_property(init, "signal");
            if !s.is_undefined() {
                signal_val = s;
            }
            let b = vm.get_property(init, "body");
            if !b.is_undefined() {
                vm.set_property(instance, "body", b);
            }
        }

        let u_val = vm.alloc_string(url_str);
        let m_val = vm.alloc_string(method_str);
        vm.set_property(instance, "url", u_val);
        vm.set_property(instance, "method", m_val);
        vm.set_property(instance, "headers", headers_obj);
        vm.set_property(instance, "signal", signal_val);

        Ok(instance)
    });
    vm.set_property(request_ctor, "prototype", request_proto);
    vm.globals.insert("Request".to_string(), request_ctor);
}

fn register_response(vm: &mut Vm) {
    let response_proto = vm.alloc_object();

    let text_fn = vm.alloc_native_function("text", 0, |vm, this, _args| {
        let b = vm.get_property(this, "__raw_body__");
        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

        if !b.is_undefined() {
            vm.resolve_promise(promise, b);
            return Ok(promise);
        }

        let stream = vm.get_property(this, "body");
        if stream.is_js_object() {
            let read_all_script = r#"
                (function(stream) {
                    var reader = stream.getReader();
                    var text = "";
                    var decoder = new TextDecoder();
                    function pump() {
                        return reader.read().then(function(res) {
                            if (res.done) return text;
                            if (typeof res.value === "string") {
                                text += res.value;
                            } else {
                                text += decoder.decode(res.value);
                            }
                            return pump();
                        });
                    }
                    return pump();
                })
            "#;
            let read_all = vm.eval(read_all_script)?;
            return vm.call(read_all, JsValue::undefined(), &[stream]);
        }

        let empty = vm.alloc_string(String::new());
        vm.resolve_promise(promise, empty);
        Ok(promise)
    });
    vm.set_property(response_proto, "text", text_fn);

    let json_fn = vm.alloc_native_function("json", 0, |vm, this, _args| {
        let text_method = vm.get_property(this, "text");
        let text_promise = vm.call(text_method, this, &[])?;

        let json_parse_script = r#"
            (function(textProm) {
                return textProm.then(function(t) {
                    return JSON.parse(t);
                });
            })
        "#;
        let jp_fn = vm.eval(json_parse_script)?;
        vm.call(jp_fn, JsValue::undefined(), &[text_promise])
    });
    vm.set_property(response_proto, "json", json_fn);

    let ab_fn = vm.alloc_native_function("arrayBuffer", 0, |vm, this, _args| {
        let raw_bytes_val = vm.get_property(this, "__raw_bytes__");
        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

        if let Some(ptr) = raw_bytes_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::ArrayBuffer(_) = &obj.kind {
                vm.resolve_promise(promise, raw_bytes_val);
                return Ok(promise);
            }
        }

        let b = vm.get_property(this, "__raw_body__");
        let s = if !b.is_undefined() { vm.to_string_value(b) } else { String::new() };
        let bytes = s.as_bytes();
        let sab = doms_parallel::SharedArrayBuffer::try_new(bytes.len())
            .map_err(|e| VmError::RuntimeError(e.into()))?;
        for (i, &byte) in bytes.iter().enumerate() {
            let _ = sab.set(i, byte);
        }
        let ab_val = vm.alloc_array_buffer(Arc::new(sab));
        vm.resolve_promise(promise, ab_val);
        Ok(promise)
    });
    vm.set_property(response_proto, "arrayBuffer", ab_fn);

    let bytes_fn = vm.alloc_native_function("bytes", 0, |vm, this, _args| {
        let ab_method = vm.get_property(this, "arrayBuffer");
        let ab_prom = vm.call(ab_method, this, &[])?;

        let make_u8_script = r#"
            (function(abProm) {
                return abProm.then(function(ab) {
                    return new Uint8Array(ab);
                });
            })
        "#;
        let u8_fn = vm.eval(make_u8_script)?;
        vm.call(u8_fn, JsValue::undefined(), &[ab_prom])
    });
    vm.set_property(response_proto, "bytes", bytes_fn);

    let clone_fn = vm.alloc_native_function("clone", 0, |vm, this, _args| {
        let resp_ctor = vm.globals.get("Response").copied().unwrap_or(JsValue::undefined());
        let body = vm.get_property(this, "body");
        let new_body = if body.is_js_object() {
            let tee_fn = vm.get_property(body, "tee");
            let branches = vm.call(tee_fn, body, &[])?;
            let b0 = vm.get_element(branches, 0);
            let b1 = vm.get_element(branches, 1);
            vm.set_property(this, "body", b0);
            b1
        } else {
            vm.get_property(this, "__raw_body__")
        };

        let init = vm.alloc_object();
        let status = vm.get_property(this, "status");
        let status_text = vm.get_property(this, "statusText");
        let headers = vm.get_property(this, "headers");
        vm.set_property(init, "status", status);
        vm.set_property(init, "statusText", status_text);
        vm.set_property(init, "headers", headers);

        vm.call(resp_ctor, JsValue::undefined(), &[new_body, init])
    });
    vm.set_property(response_proto, "clone", clone_fn);

    let response_ctor = vm.alloc_native_function("Response", 2, |vm, _this, args| {
        let body_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let init = args.get(1).copied().unwrap_or(JsValue::undefined());

        let ctor = vm.globals.get("Response").copied().unwrap_or(JsValue::undefined());
        let proto = vm.get_property(ctor, "prototype");
        let instance = vm.alloc_object();
        vm.set_prototype(instance, proto);

        let mut status = 200;
        let mut status_text = "OK".to_string();
        let headers_ctor = vm.globals.get("Headers").copied().unwrap_or(JsValue::undefined());
        let mut headers_obj = vm.call(headers_ctor, JsValue::undefined(), &[])?;

        if init.is_js_object() {
            let st = vm.get_property(init, "status");
            if let Some(num) = st.to_int32() {
                status = num;
            }
            let st_text = vm.get_property(init, "statusText");
            if !st_text.is_undefined() {
                status_text = vm.to_string_value(st_text);
            }
            let h_init = vm.get_property(init, "headers");
            if !h_init.is_undefined() {
                headers_obj = vm.call(headers_ctor, JsValue::undefined(), &[h_init])?;
            }
        }

        vm.set_property(instance, "status", JsValue::from_int32(status));
        let st_text_val = vm.alloc_string(status_text);
        vm.set_property(instance, "statusText", st_text_val);
        let ok = (200..300).contains(&status);
        vm.set_property(instance, "ok", JsValue::from_boolean(ok));
        vm.set_property(instance, "headers", headers_obj);
        vm.set_property(instance, "bodyUsed", JsValue::from_boolean(false));

        if body_arg.is_js_object() {
            let get_reader = vm.get_property(body_arg, "getReader");
            if vm.is_callable(get_reader) {
                vm.set_property(instance, "body", body_arg);
            } else {
                let body_str = vm.to_string_value(body_arg);
                let body_val = vm.alloc_string(body_str);
                vm.set_property(instance, "__raw_body__", body_val);
                vm.set_property(instance, "body", JsValue::null());
            }
        } else if !body_arg.is_undefined() {
            let body_str = vm.to_string_value(body_arg);
            let body_val = vm.alloc_string(body_str);
            vm.set_property(instance, "__raw_body__", body_val);
            vm.set_property(instance, "body", JsValue::null());
        } else {
            vm.set_property(instance, "__raw_body__", JsValue::undefined());
            vm.set_property(instance, "body", JsValue::null());
        }

        Ok(instance)
    });

    // Static Response.json(data, init)
    let static_json = vm.alloc_native_function("json", 2, |vm, _this, args| {
        let data = args.first().copied().unwrap_or(JsValue::undefined());
        let init = args.get(1).copied().unwrap_or(JsValue::undefined());

        let json_global = vm.globals.get("JSON").copied().unwrap_or(JsValue::undefined());
        let stringify_fn = vm.get_property(json_global, "stringify");
        let body_str = vm.call(stringify_fn, json_global, &[data])?;

        let resp_init = if init.is_js_object() { init } else { vm.alloc_object() };
        let headers = vm.get_property(resp_init, "headers");
        let headers_ctor = vm.globals.get("Headers").copied().unwrap_or(JsValue::undefined());
        let headers_obj = vm.call(headers_ctor, JsValue::undefined(), &[headers])?;
        let ct_name = vm.alloc_string("content-type".into());
        let ct_val = vm.alloc_string("application/json".into());
        let set_fn = vm.get_property(headers_obj, "set");
        let _ = vm.call(set_fn, headers_obj, &[ct_name, ct_val]);
        vm.set_property(resp_init, "headers", headers_obj);

        let resp_ctor = vm.globals.get("Response").copied().unwrap_or(JsValue::undefined());
        vm.call(resp_ctor, JsValue::undefined(), &[body_str, resp_init])
    });
    vm.set_property(response_ctor, "json", static_json);

    // Static Response.error()
    let static_error = vm.alloc_native_function("error", 0, |vm, _this, _args| {
        let resp_ctor = vm.globals.get("Response").copied().unwrap_or(JsValue::undefined());
        let init = vm.alloc_object();
        vm.set_property(init, "status", JsValue::from_int32(0));
        let empty_str = vm.alloc_string("".into());
        vm.set_property(init, "statusText", empty_str);
        let resp = vm.call(resp_ctor, JsValue::undefined(), &[JsValue::undefined(), init])?;
        let type_val = vm.alloc_string("error".into());
        vm.set_property(resp, "type", type_val);
        Ok(resp)
    });
    vm.set_property(response_ctor, "error", static_error);

    // Static Response.redirect(url, status)
    let static_redirect = vm.alloc_native_function("redirect", 2, |vm, _this, args| {
        let url_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let status_arg = args.get(1).copied().unwrap_or(JsValue::from_int32(302));
        let status = status_arg.to_int32().unwrap_or(302);
        if ![301, 302, 303, 307, 308].contains(&status) {
            return Err(VmError::RangeError("Invalid redirect status code".into()));
        }

        let resp_ctor = vm.globals.get("Response").copied().unwrap_or(JsValue::undefined());
        let init = vm.alloc_object();
        vm.set_property(init, "status", JsValue::from_int32(status));
        let headers_ctor = vm.globals.get("Headers").copied().unwrap_or(JsValue::undefined());
        let headers_obj = vm.call(headers_ctor, JsValue::undefined(), &[])?;
        let loc_name = vm.alloc_string("location".into());
        let set_fn = vm.get_property(headers_obj, "set");
        let _ = vm.call(set_fn, headers_obj, &[loc_name, url_arg]);
        vm.set_property(init, "headers", headers_obj);

        vm.call(resp_ctor, JsValue::undefined(), &[JsValue::undefined(), init])
    });
    vm.set_property(response_ctor, "redirect", static_redirect);

    vm.set_property(response_ctor, "prototype", response_proto);
    vm.globals.insert("Response".to_string(), response_ctor);
}

fn register_fetch_fn(vm: &mut Vm) {
    let fetch_fn = vm.alloc_native_function("fetch", 2, |vm, _this, args| {
        let input = args.first().copied().unwrap_or(JsValue::undefined());
        let init = args.get(1).copied().unwrap_or(JsValue::undefined());

        let url_str = if input.is_js_object() {
            let u = vm.get_property(input, "url");
            vm.to_string_value(u)
        } else {
            vm.to_string_value(input)
        };
        let mut method = "GET".to_string();
        let mut headers_vec: Vec<(String, String)> = Vec::new();
        let mut body_bytes: Option<Vec<u8>> = None;
        let mut signal_val = JsValue::undefined();
        let mut redirect_mode = "follow".to_string();

        if input.is_js_object() {
            let m = vm.get_property(input, "method");
            if !m.is_undefined() {
                method = vm.to_string_value(m).to_ascii_uppercase();
            }
            let s = vm.get_property(input, "signal");
            if !s.is_undefined() {
                signal_val = s;
            }
        }

        if init.is_js_object() {
            let m = vm.get_property(init, "method");
            if !m.is_undefined() {
                method = vm.to_string_value(m).to_ascii_uppercase();
            }
            let s = vm.get_property(init, "signal");
            if !s.is_undefined() {
                signal_val = s;
            }
            let r = vm.get_property(init, "redirect");
            if !r.is_undefined() {
                redirect_mode = vm.to_string_value(r);
            }
            let b = vm.get_property(init, "body");
            if !b.is_undefined() {
                let bs = vm.to_string_value(b);
                body_bytes = Some(bs.into_bytes());
            }

            // Extract headers
            let h = vm.get_property(init, "headers");
            if h.is_js_object() {
                let store = vm.get_property(h, "__store__");
                let target = if store.is_js_object() { store } else { h };
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &*(ptr as *mut JsObject) };
                    for (k, offset) in vm.shapes.all_property_entries(obj.shape_id) {
                        if let Some(v) = obj.get_slot(offset) {
                            headers_vec.push((k, vm.to_string_value(v)));
                        }
                    }
                }
            }
        }

        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

        // Check if signal is already aborted
        if signal_val.is_js_object() {
            let aborted = vm.get_property(signal_val, "__aborted__");
            if aborted.is_truthy() {
                let reason = vm.get_property(signal_val, "__reason__");
                vm.reject_promise(promise, reason);
                return Ok(promise);
            }
        }

        // Handle data: URIs
        if let Some(stripped) = url_str.strip_prefix("data:") {
            let content = if let Some((_, b)) = stripped.split_once(',') {
                b.to_string()
            } else {
                stripped.to_string()
            };
            let body_val = vm.alloc_string(content);
            let resp_init = vm.alloc_object();
            vm.set_property(resp_init, "status", JsValue::from_int32(200));
            let ok_str = vm.alloc_string("OK".into());
            vm.set_property(resp_init, "statusText", ok_str);

            let resp_ctor = vm.globals.get("Response").copied().unwrap_or(JsValue::undefined());
            let resp_obj = vm.call(resp_ctor, JsValue::undefined(), &[body_val, resp_init])?;
            let url_val = vm.alloc_string(url_str);
            vm.set_property(resp_obj, "url", url_val);

            vm.resolve_promise(promise, resp_obj);
            return Ok(promise);
        }

        // Execute real HTTP/HTTPS fetch via ureq
        let max_redirects = match redirect_mode.as_str() {
            "manual" => 0,
            _ => 10,
        };

        let config = ureq::config::Config::builder().max_redirects(max_redirects).build();
        let agent: ureq::Agent = config.into();

        let mut builder = ureq::http::Request::builder().method(method.as_str()).uri(&url_str);

        for (k, v) in headers_vec {
            builder = builder.header(k, v);
        }

        let send_body = body_bytes.unwrap_or_default();
        let http_req = match builder.body(send_body) {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("TypeError: invalid request: {}", e);
                let err_val = vm.alloc_string(err_msg);
                vm.reject_promise(promise, err_val);
                return Ok(promise);
            }
        };

        let resp_result = agent.run(http_req);

        match resp_result {
            Ok(ureq_resp) => {
                let status_code = ureq_resp.status().as_u16() as i32;
                let status_text = ureq_resp.status().canonical_reason().unwrap_or("OK").to_string();

                let headers_ctor =
                    vm.globals.get("Headers").copied().unwrap_or(JsValue::undefined());
                let headers_obj = vm.call(headers_ctor, JsValue::undefined(), &[])?;
                let set_fn = vm.get_property(headers_obj, "set");

                for (name, val) in ureq_resp.headers() {
                    let k_val = vm.alloc_string(name.as_str().to_string());
                    let v_val = vm.alloc_string(val.to_str().unwrap_or("").to_string());
                    let _ = vm.call(set_fn, headers_obj, &[k_val, v_val]);
                }

                // Read body chunks
                let mut reader = ureq_resp.into_body().into_reader();
                let mut buffer = Vec::new();
                let _ = reader.read_to_end(&mut buffer);

                // Build ReadableStream
                let rs_ctor =
                    vm.globals.get("ReadableStream").copied().unwrap_or(JsValue::undefined());
                let chunks_arr = vm.alloc_array(Vec::new());
                let push_chunk = vm.get_property(chunks_arr, "push");

                // Chunk size: 16KB
                for chunk in buffer.chunks(16384) {
                    let sab = doms_parallel::SharedArrayBuffer::try_new(chunk.len())
                        .map_err(|e| VmError::RuntimeError(e.into()))?;
                    for (i, &b) in chunk.iter().enumerate() {
                        let _ = sab.set(i, b);
                    }
                    let ta_val = vm.alloc_typed_array(JsTypedArray {
                        buffer: Arc::new(sab),
                        buffer_val: None,
                        kind: TypedArrayKind::Uint8,
                        length: chunk.len(),
                        byte_offset: 0,
                    });
                    let _ = vm.call(push_chunk, chunks_arr, &[ta_val]);
                }

                let stream_source_script = r#"
                    (function(chunks) {
                        return {
                            start: function(controller) {
                                for (var i = 0; i < chunks.length; i++) {
                                    controller.enqueue(chunks[i]);
                                }
                                controller.close();
                            }
                        };
                    })
                "#;
                let make_src = vm.eval(stream_source_script)?;
                let src_obj = vm.call(make_src, JsValue::undefined(), &[chunks_arr])?;
                let stream_obj = vm.call(rs_ctor, JsValue::undefined(), &[src_obj])?;

                let resp_init = vm.alloc_object();
                vm.set_property(resp_init, "status", JsValue::from_int32(status_code));
                let st_val = vm.alloc_string(status_text);
                vm.set_property(resp_init, "statusText", st_val);
                vm.set_property(resp_init, "headers", headers_obj);

                let resp_ctor = vm.globals.get("Response").copied().unwrap_or(JsValue::undefined());
                let resp_obj =
                    vm.call(resp_ctor, JsValue::undefined(), &[stream_obj, resp_init])?;
                let u_val = vm.alloc_string(url_str);
                vm.set_property(resp_obj, "url", u_val);

                // Cache raw body as string for quick text()/json()
                let raw_text = String::from_utf8_lossy(&buffer).into_owned();
                let raw_val = vm.alloc_string(raw_text);
                vm.set_property(resp_obj, "__raw_body__", raw_val);

                vm.resolve_promise(promise, resp_obj);
            }
            Err(e) => {
                let err_msg = format!("TypeError: fetch failed: {}", e);
                let err_val = vm.alloc_string(err_msg);
                vm.reject_promise(promise, err_val);
            }
        }

        Ok(promise)
    });
    vm.globals.insert("fetch".to_string(), fetch_fn);
}
