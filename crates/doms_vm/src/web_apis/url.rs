//! `URL` and `URLSearchParams`.

use super::*;

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

pub(super) fn register_url(vm: &mut Vm) {
    let usp_ctor = vm.alloc_native_function("URLSearchParams", 1, |vm, _this, args| {
        let query_str = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let pairs = parse_query_string(&query_str);
        let instance = vm.alloc_object();

        let ser = pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
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
            let new_ser =
                pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
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
            let new_ser =
                pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
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
            let new_ser =
                pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
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
