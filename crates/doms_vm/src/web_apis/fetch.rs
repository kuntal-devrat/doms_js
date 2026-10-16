//! `Headers`, `Response` and `fetch`.

use super::*;

pub(super) fn register_fetch(vm: &mut Vm) {
    let headers_ctor = vm.alloc_native_function("Headers", 1, |vm, _this, args| {
        let instance = vm.alloc_object();
        let store = vm.alloc_object();
        vm.set_property(instance, "__store__", store);

        let init = args.first().copied().unwrap_or(JsValue::undefined());
        if init.is_object() {
            if let Some(ptr) = init.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                let pairs: Vec<(String, JsValue)> = vm
                    .shapes
                    .all_property_entries(obj.shape_id)
                    .into_iter()
                    .filter_map(|(k, offset)| obj.get_slot(offset).map(|v| (k, v)))
                    .collect();
                for (k, v) in pairs {
                    vm.set_property(store, &k.to_ascii_lowercase(), v);
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
                    let pairs: Vec<(String, JsValue)> = vm
                        .shapes
                        .all_property_entries(obj.shape_id)
                        .into_iter()
                        .filter_map(|(k, offset)| obj.get_slot(offset).map(|v| (k, v)))
                        .collect();
                    let store = vm.get_property(headers_obj, "__store__");
                    for (k, v) in pairs {
                        vm.set_property(store, &k.to_ascii_lowercase(), v);
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
