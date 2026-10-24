//! Tests for the web standard host APIs.

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
fn test_fetch_data_uri_response() {
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

#[test]
fn test_web_crypto_random_uuid() {
    let mut vm = Vm::new();
    let script = r#"
        var uuid = crypto.randomUUID();
        var isString = typeof uuid === "string";
        var lengthValid = uuid.length === 36;
        var versionChar = uuid.charAt(14);
        var variantChar = uuid.charAt(19);
    "#;
    vm.eval(script).expect("eval should succeed");
    assert_eq!(vm.globals.get("isString").map(|v| v.is_truthy()), Some(true));
    assert_eq!(vm.globals.get("lengthValid").map(|v| v.is_truthy()), Some(true));
    assert_eq!(vm.to_string_value(vm.globals.get("versionChar").copied().unwrap()), "4");
    let variant = vm.to_string_value(vm.globals.get("variantChar").copied().unwrap());
    assert!(["8", "9", "a", "b"].contains(&variant.as_str()));
}

#[test]
fn test_web_crypto_get_random_values() {
    let mut vm = Vm::new();
    let script = r#"
        var arr = new Uint8Array(32);
        crypto.getRandomValues(arr);
        var sum = 0;
        for (var i = 0; i < arr.length; i++) {
            sum += arr[i];
        }
        var hasNonZero = sum > 0;
    "#;
    vm.eval(script).expect("eval should succeed");
    assert_eq!(vm.globals.get("hasNonZero").map(|v| v.is_truthy()), Some(true));
}

#[test]
fn test_web_crypto_subtle_digest_sha256() {
    let mut vm = Vm::new();
    let script = r#"
        var digestHex = null;
        var encoder = new TextEncoder();
        var data = encoder.encode("hello world");
        crypto.subtle.digest("SHA-256", data).then(function(buf) {
            var bytes = new Uint8Array(buf);
            var hex = "";
            for (var i = 0; i < bytes.length; i++) {
                var b = bytes[i].toString(16);
                if (b.length < 2) b = "0" + b;
                hex += b;
            }
            digestHex = hex;
        });
    "#;
    vm.eval(script).expect("eval should succeed");
    vm.drain_microtasks().expect("drain microtasks");
    let hex = vm.globals.get("digestHex").copied().unwrap();
    // Known SHA-256 test vector for "hello world"
    assert_eq!(
        vm.to_string_value(hex),
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
}

#[test]
fn test_readable_stream_and_reader() {
    let mut vm = Vm::new();
    let script = r#"
        var stream = new ReadableStream({
            start: function(controller) {
                controller.enqueue("chunk1");
                controller.enqueue("chunk2");
                controller.close();
            }
        });

        var chunks = [];
        var reader = stream.getReader();
        reader.read().then(function(r1) {
            chunks.push(r1.value);
            reader.read().then(function(r2) {
                chunks.push(r2.value);
                reader.read().then(function(r3) {
                    chunks.push(r3.done);
                });
            });
        });
    "#;
    vm.eval(script).expect("eval should succeed");
    vm.drain_microtasks().expect("drain microtasks");
    let chunks_val = vm.globals.get("chunks").copied().unwrap();
    let c0 = vm.get_element(chunks_val, 0);
    assert_eq!(vm.to_string_value(c0), "chunk1");
    let c1 = vm.get_element(chunks_val, 1);
    assert_eq!(vm.to_string_value(c1), "chunk2");
    let c2 = vm.get_element(chunks_val, 2);
    assert!(c2.is_truthy());
}

#[test]
fn test_readable_stream_piping() {
    let mut vm = Vm::new();
    let script = r#"
        var received = [];
        var rs = new ReadableStream({
            start: function(controller) {
                controller.enqueue(10);
                controller.enqueue(20);
                controller.close();
            }
        });
        var ws = new WritableStream({
            write: function(chunk) {
                received.push(chunk);
            }
        });
        var pipeFinished = false;
        rs.pipeTo(ws).then(function() {
            pipeFinished = true;
        });
    "#;
    vm.eval(script).expect("eval should succeed");
    vm.drain_microtasks().expect("drain microtasks");
    assert_eq!(vm.globals.get("pipeFinished").map(|v| v.is_truthy()), Some(true));
    let rec_val = vm.globals.get("received").copied().unwrap();
    assert_eq!(vm.get_element(rec_val, 0).to_int32(), Some(10));
    assert_eq!(vm.get_element(rec_val, 1).to_int32(), Some(20));
}

#[test]
fn test_transform_stream() {
    let mut vm = Vm::new();
    let script = r#"
        var ts = new TransformStream({
            transform: function(chunk, controller) {
                controller.enqueue(chunk * 2);
            }
        });

        var results = [];
        var reader = ts.readable.getReader();
        var writer = ts.writable.getWriter();

        writer.write(5);
        writer.write(15);
        writer.close();

        reader.read().then(function(r1) {
            results.push(r1.value);
            return reader.read();
        }).then(function(r2) {
            results.push(r2.value);
        });
    "#;
    vm.eval(script).expect("eval should succeed");
    vm.drain_microtasks().expect("drain microtasks");
    let res = vm.globals.get("results").copied().unwrap();
    assert_eq!(vm.get_element(res, 0).to_int32(), Some(10));
    assert_eq!(vm.get_element(res, 1).to_int32(), Some(30));
}

#[test]
fn test_headers_methods() {
    let mut vm = Vm::new();
    let script = r#"
        var h = new Headers({ "Content-Type": "application/json" });
        h.append("Accept", "text/html");
        h.append("Accept", "application/xhtml+xml");
        var ct = h.get("content-type");
        var accept = h.get("accept");
        var hasAuth = h.has("authorization");
        h.set("Authorization", "Bearer token123");
        var hasAuthNow = h.has("authorization");
    "#;
    vm.eval(script).expect("eval should succeed");
    assert_eq!(vm.to_string_value(vm.globals.get("ct").copied().unwrap()), "application/json");
    assert_eq!(
        vm.to_string_value(vm.globals.get("accept").copied().unwrap()),
        "text/html, application/xhtml+xml"
    );
    assert_eq!(vm.globals.get("hasAuth").map(|v| v.is_truthy()), Some(false));
    assert_eq!(vm.globals.get("hasAuthNow").map(|v| v.is_truthy()), Some(true));
}

#[test]
fn test_abort_controller_and_signal() {
    let mut vm = Vm::new();
    let script = r#"
        var controller = new AbortController();
        var signal = controller.signal;
        var wasAbortedBefore = signal.aborted;
        var eventFired = false;
        signal.addEventListener("abort", function() {
            eventFired = true;
        });
        controller.abort("manual abort");
        var wasAbortedAfter = signal.aborted;
        var reason = signal.reason;
    "#;
    vm.eval(script).expect("eval should succeed");
    assert_eq!(vm.globals.get("wasAbortedBefore").map(|v| v.is_truthy()), Some(false));
    assert_eq!(vm.globals.get("wasAbortedAfter").map(|v| v.is_truthy()), Some(true));
    assert_eq!(vm.globals.get("eventFired").map(|v| v.is_truthy()), Some(true));
    assert_eq!(vm.to_string_value(vm.globals.get("reason").copied().unwrap()), "manual abort");
}

#[test]
fn test_response_json_and_clone() {
    let mut vm = Vm::new();
    let script = r#"
        var resp = Response.json({ status: "success", count: 42 });
        var status = resp.status;
        var ok = resp.ok;
        var jsonResult = null;
        resp.json().then(function(data) {
            jsonResult = data;
        });
    "#;
    vm.eval(script).expect("eval should succeed");
    vm.drain_microtasks().expect("drain microtasks");
    assert_eq!(vm.globals.get("status").and_then(|v| v.to_int32()), Some(200));
    assert_eq!(vm.globals.get("ok").map(|v| v.is_truthy()), Some(true));
    let jr = vm.globals.get("jsonResult").copied().unwrap();
    let status_prop = vm.get_property(jr, "status");
    assert_eq!(vm.to_string_value(status_prop), "success");
    let count_prop = vm.get_property(jr, "count");
    assert_eq!(count_prop.to_int32(), Some(42));
}

#[test]
fn test_url_relative_without_base_throws_type_error() {
    let mut vm = Vm::new();
    let res1 = vm.eval(r#"new URL("relative_path")"#);
    assert!(res1.is_err(), "new URL without base must throw TypeError");

    let res2 = vm.eval(r#"new URL("subpath", "relative_base")"#);
    assert!(res2.is_err(), "new URL with invalid base must throw TypeError");

    let res3 = vm.eval(r#"new URL("/foo", "https://example.com/bar").href"#).unwrap();
    assert_eq!(vm.to_string_value(res3), "https://example.com/foo");

    let res4 = vm.eval(r#"new URL("foo", "https://example.com/bar/").href"#).unwrap();
    assert_eq!(vm.to_string_value(res4), "https://example.com/bar/foo");
}

#[test]
fn test_url_search_params_methods_and_decoding() {
    let mut vm = Vm::new();
    let script = r#"
        var usp = new URLSearchParams("query=hello+world&tag=foo&query=hello%20again");
        var qAll = usp.getAll("query");
        var qSingle = usp.get("query");
        var decodedSpace = qSingle === "hello world";

        var usp2 = new URLSearchParams("z=9&a=1&m=5");
        usp2.sort();
        var sortedStr = usp2.toString();

        var iterated = [];
        usp2.forEach(function(val, key) {
            iterated.push(key + ":" + val);
        });
        var forEachResult = iterated.join(",");
    "#;
    vm.eval(script).expect("eval should succeed");
    assert_eq!(vm.globals.get("decodedSpace").map(|v| v.is_truthy()), Some(true));
    let q_all = vm.globals.get("qAll").copied().unwrap();
    assert!(q_all.is_object());
    let len = vm.get_property(q_all, "length");
    assert_eq!(len.as_number(), Some(2.0));
    let q0 = vm.get_property(q_all, "0");
    assert_eq!(vm.to_string_value(q0), "hello world");
    let q1 = vm.get_property(q_all, "1");
    assert_eq!(vm.to_string_value(q1), "hello again");

    assert_eq!(vm.to_string_value(vm.globals.get("sortedStr").copied().unwrap()), "a=1&m=5&z=9");
    assert_eq!(vm.to_string_value(vm.globals.get("forEachResult").copied().unwrap()), "a:1,m:5,z:9");
}

#[test]
fn test_request_prototype_methods_and_clone() {
    let mut vm = Vm::new();
    let script = r#"
        var req = new Request("https://example.com/api", {
            method: "POST",
            body: "test body content"
        });
        var initialBodyUsed = req.bodyUsed;
        var cloned = req.clone();
        var clonedMethod = cloned.method;

        var textResult = null;
        req.text().then(function(t) {
            textResult = t;
        });

        var afterTextBodyUsed = req.bodyUsed;
        var cloneFailed = false;
        try {
            req.clone();
        } catch (e) {
            cloneFailed = true;
        }
    "#;
    vm.eval(script).expect("eval should succeed");
    vm.drain_microtasks().expect("drain microtasks");

    assert_eq!(vm.globals.get("initialBodyUsed").map(|v| v.is_truthy()), Some(false));
    assert_eq!(vm.to_string_value(vm.globals.get("clonedMethod").copied().unwrap()), "POST");
    assert_eq!(vm.globals.get("afterTextBodyUsed").map(|v| v.is_truthy()), Some(true));
    assert_eq!(vm.globals.get("cloneFailed").map(|v| v.is_truthy()), Some(true));
    assert_eq!(vm.to_string_value(vm.globals.get("textResult").copied().unwrap()), "test body content");
}

#[test]
fn test_abort_signal_timeout_without_eval() {
    let mut vm = Vm::new();
    let script = r#"
        var sig = AbortSignal.timeout(10);
        var initialAborted = sig.aborted;
    "#;
    vm.eval(script).expect("eval should succeed");
    assert_eq!(vm.globals.get("initialAborted").map(|v| v.is_truthy()), Some(false));

    std::thread::sleep(Duration::from_millis(25));
    vm.run_event_loop().expect("run event loop");

    let sig = vm.globals.get("sig").copied().unwrap();
    let aborted = vm.get_property(sig, "aborted");
    assert_eq!(aborted.is_truthy(), true);
    let reason = vm.get_property(sig, "reason");
    let name = vm.get_property(reason, "name");
    assert_eq!(vm.to_string_value(name), "TimeoutError");
}

#[test]
fn test_fetch_with_request_instance() {
    let mut vm = Vm::new();
    let script = r#"
        var req = new Request("data:text/plain,Passed%20Through%20Request");
        var fetchResult = null;
        fetch(req).then(function(res) {
            return res.text();
        }).then(function(t) {
            fetchResult = t;
        });
    "#;
    vm.eval(script).expect("eval should succeed");
    vm.drain_microtasks().expect("drain microtasks");
    let res = vm.globals.get("fetchResult").copied().unwrap();
    assert_eq!(vm.to_string_value(res), "Passed%20Through%20Request");
}
