//! WHATWG Streams Standard (`ReadableStream`, `WritableStream`, `TransformStream`).

use super::*;

pub(super) fn register_streams(vm: &mut Vm) {
    register_readable_stream(vm);
    register_writable_stream(vm);
    register_transform_stream(vm);
    register_queuing_strategies(vm);
}

fn register_readable_stream(vm: &mut Vm) {
    // 1. ReadableStreamDefaultReader prototype
    let reader_proto = vm.alloc_object();

    let reader_read = vm.alloc_native_function("read", 0, |vm, this, _args| {
        let stream = vm.get_property(this, "__stream__");
        if !stream.is_js_object() {
            return Err(VmError::TypeError(
                "ReadableStreamDefaultReader.read called on invalid reader".into(),
            ));
        }

        let state = vm.get_property(stream, "__state__");
        let state_str = vm.to_string_value(state);

        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

        if state_str == "errored" {
            let err = vm.get_property(stream, "__stored_error__");
            vm.reject_promise(promise, err);
            return Ok(promise);
        }

        let queue = vm.get_property(stream, "__queue__");
        let queue_len_val = vm.get_property(queue, "length");
        let queue_len = queue_len_val.to_int32().unwrap_or(0);

        if queue_len > 0 {
            // Shift front chunk
            let shift_fn = vm.get_property(queue, "shift");
            let chunk = vm.call(shift_fn, queue, &[])?;

            let result_obj = vm.alloc_object();
            vm.set_property(result_obj, "value", chunk);
            vm.set_property(result_obj, "done", JsValue::from_boolean(false));
            vm.resolve_promise(promise, result_obj);
            return Ok(promise);
        }

        if state_str == "closed" {
            let result_obj = vm.alloc_object();
            vm.set_property(result_obj, "value", JsValue::undefined());
            vm.set_property(result_obj, "done", JsValue::from_boolean(true));
            vm.resolve_promise(promise, result_obj);
            return Ok(promise);
        }

        // Readable but queue is empty -> store pending read request
        let pending_reads = vm.get_property(this, "__pending_reads__");
        let push_fn = vm.get_property(pending_reads, "push");
        vm.call(push_fn, pending_reads, &[promise])?;

        // Invoke underlyingSource.pull(controller)
        let source = vm.get_property(stream, "__source__");
        if source.is_js_object() {
            let pull_fn = vm.get_property(source, "pull");
            if vm.is_callable(pull_fn) {
                let controller = vm.get_property(stream, "__controller__");
                let _ = vm.call(pull_fn, source, &[controller]);
            }
        }

        Ok(promise)
    });
    vm.set_property(reader_proto, "read", reader_read);

    let reader_release_lock = vm.alloc_native_function("releaseLock", 0, |vm, this, _args| {
        let stream = vm.get_property(this, "__stream__");
        if stream.is_js_object() {
            vm.set_property(stream, "__reader__", JsValue::undefined());
            vm.set_property(this, "__stream__", JsValue::undefined());
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(reader_proto, "releaseLock", reader_release_lock);

    let reader_cancel = vm.alloc_native_function("cancel", 1, |vm, this, args| {
        let stream = vm.get_property(this, "__stream__");
        if !stream.is_js_object() {
            return Err(VmError::TypeError("Reader not attached to stream".into()));
        }
        let cancel_fn = vm.get_property(stream, "cancel");
        vm.call(cancel_fn, stream, args)
    });
    vm.set_property(reader_proto, "cancel", reader_cancel);

    let reader_closed_getter = vm.alloc_native_function("get closed", 0, |vm, this, _args| {
        let closed_prom = vm.get_property(this, "__closed_promise__");
        Ok(closed_prom)
    });
    if let Some(ptr) = reader_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor("closed".to_string(), Some(reader_closed_getter), None);
    }

    let reader_ctor =
        vm.alloc_native_function("ReadableStreamDefaultReader", 1, |vm, _this, args| {
            let stream = args.first().copied().unwrap_or(JsValue::undefined());
            if !stream.is_js_object() {
                return Err(VmError::TypeError(
                    "ReadableStreamDefaultReader constructor requires a ReadableStream".into(),
                ));
            }

            let existing_reader = vm.get_property(stream, "__reader__");
            if !existing_reader.is_undefined() {
                return Err(VmError::TypeError(
                    "ReadableStream is already locked by another reader".into(),
                ));
            }

            let ctor = vm
                .globals
                .get("ReadableStreamDefaultReader")
                .copied()
                .unwrap_or(JsValue::undefined());
            let proto = vm.get_property(ctor, "prototype");

            let instance = vm.alloc_object();
            vm.set_prototype(instance, proto);

            vm.set_property(instance, "__stream__", stream);
            let pending = vm.alloc_array(Vec::new());
            vm.set_property(instance, "__pending_reads__", pending);

            let closed_promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            let stream_state = vm.get_property(stream, "__state__");
            let stream_state_str = vm.to_string_value(stream_state);
            if stream_state_str == "closed" {
                vm.resolve_promise(closed_promise, JsValue::undefined());
            } else if stream_state_str == "errored" {
                let err = vm.get_property(stream, "__stored_error__");
                vm.reject_promise(closed_promise, err);
            }
            vm.set_property(instance, "__closed_promise__", closed_promise);

            vm.set_property(stream, "__reader__", instance);
            Ok(instance)
        });
    vm.set_property(reader_ctor, "prototype", reader_proto);
    vm.globals.insert("ReadableStreamDefaultReader".to_string(), reader_ctor);

    // 2. ReadableStream prototype
    let stream_proto = vm.alloc_object();

    let stream_locked_getter = vm.alloc_native_function("get locked", 0, |vm, this, _args| {
        let reader = vm.get_property(this, "__reader__");
        Ok(JsValue::from_boolean(!reader.is_undefined()))
    });
    if let Some(ptr) = stream_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor("locked".to_string(), Some(stream_locked_getter), None);
    }

    let stream_get_reader = vm.alloc_native_function("getReader", 0, |vm, this, _args| {
        let ctor =
            vm.globals.get("ReadableStreamDefaultReader").copied().unwrap_or(JsValue::undefined());
        vm.call(ctor, JsValue::undefined(), &[this])
    });
    vm.set_property(stream_proto, "getReader", stream_get_reader);

    let stream_cancel = vm.alloc_native_function("cancel", 1, |vm, this, args| {
        let state = vm.get_property(this, "__state__");
        let state_str = vm.to_string_value(state);
        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

        if state_str == "errored" {
            let err = vm.get_property(this, "__stored_error__");
            vm.reject_promise(promise, err);
            return Ok(promise);
        }

        if state_str == "closed" {
            vm.resolve_promise(promise, JsValue::undefined());
            return Ok(promise);
        }

        let closed_str = vm.alloc_string("closed".into());
        vm.set_property(this, "__state__", closed_str);

        let reason = args.first().copied().unwrap_or(JsValue::undefined());
        let source = vm.get_property(this, "__source__");
        if source.is_js_object() {
            let cancel_fn = vm.get_property(source, "cancel");
            if vm.is_callable(cancel_fn) {
                let _ = vm.call(cancel_fn, source, &[reason]);
            }
        }

        let reader = vm.get_property(this, "__reader__");
        if reader.is_js_object() {
            let closed_prom = vm.get_property(reader, "__closed_promise__");
            vm.resolve_promise(closed_prom, JsValue::undefined());
        }

        vm.resolve_promise(promise, JsValue::undefined());
        Ok(promise)
    });
    vm.set_property(stream_proto, "cancel", stream_cancel);

    let stream_pipe_to = vm.alloc_native_function("pipeTo", 2, |vm, this, args| {
        let dest = args.first().copied().unwrap_or(JsValue::undefined());
        if !dest.is_js_object() {
            return Err(VmError::TypeError("pipeTo requires a destination WritableStream".into()));
        }

        let pump_script = r#"
            (function(stream, dest) {
                return new Promise(function(resolve, reject) {
                    var reader = stream.getReader();
                    var writer = dest.getWriter();
                    function pump() {
                        reader.read().then(function(result) {
                            if (result.done) {
                                writer.close().then(resolve, reject);
                                return;
                            }
                            writer.write(result.value).then(pump, function(err) {
                                reject(err);
                            });
                        }, function(err) {
                            writer.abort(err).then(function() {
                                reject(err);
                            }, function() {
                                reject(err);
                            });
                        });
                    }
                    pump();
                });
            })
        "#;
        let pump_fn = vm.eval(pump_script)?;
        vm.call(pump_fn, JsValue::undefined(), &[this, dest])
    });
    vm.set_property(stream_proto, "pipeTo", stream_pipe_to);

    let stream_pipe_through = vm.alloc_native_function("pipeThrough", 2, |vm, this, args| {
        let transform = args.first().copied().unwrap_or(JsValue::undefined());
        if !transform.is_js_object() {
            return Err(VmError::TypeError(
                "pipeThrough requires an object with readable and writable properties".into(),
            ));
        }
        let writable = vm.get_property(transform, "writable");
        let readable = vm.get_property(transform, "readable");

        let pipe_to_fn = vm.get_property(this, "pipeTo");
        let _ = vm.call(pipe_to_fn, this, &[writable])?;

        Ok(readable)
    });
    vm.set_property(stream_proto, "pipeThrough", stream_pipe_through);

    let stream_tee = vm.alloc_native_function("tee", 0, |vm, this, _args| {
        let get_reader = vm.get_property(this, "getReader");
        let reader = vm.call(get_reader, this, &[])?;

        let tee_script = r#"
            (function(reader, ctor) {
                var c1, c2;
                var s1 = new ctor({
                    start: function(controller) { c1 = controller; },
                    pull: function() { return pump(); },
                    cancel: function(r) { return reader.cancel(r); }
                });
                var s2 = new ctor({
                    start: function(controller) { c2 = controller; },
                    pull: function() { return pump(); },
                    cancel: function(r) { return reader.cancel(r); }
                });
                var reading = false;
                function pump() {
                    if (reading) return;
                    reading = true;
                    return reader.read().then(function(res) {
                        reading = false;
                        if (res.done) {
                            try { c1.close(); } catch(_) {}
                            try { c2.close(); } catch(_) {}
                            return;
                        }
                        try { c1.enqueue(res.value); } catch(_) {}
                        try { c2.enqueue(res.value); } catch(_) {}
                    }, function(err) {
                        reading = false;
                        try { c1.error(err); } catch(_) {}
                        try { c2.error(err); } catch(_) {}
                    });
                }
                return [s1, s2];
            })
        "#;
        let tee_fn = vm.eval(tee_script)?;
        let ctor = vm.globals.get("ReadableStream").copied().unwrap_or(JsValue::undefined());
        vm.call(tee_fn, JsValue::undefined(), &[reader, ctor])
    });
    vm.set_property(stream_proto, "tee", stream_tee);

    let readable_ctor = vm.alloc_native_function("ReadableStream", 2, |vm, _this, args| {
        let underlying_source = args.first().copied().unwrap_or(JsValue::undefined());
        let _strategy = args.get(1).copied().unwrap_or(JsValue::undefined());

        let ctor = vm.globals.get("ReadableStream").copied().unwrap_or(JsValue::undefined());
        let proto = vm.get_property(ctor, "prototype");

        let instance = vm.alloc_object();
        vm.set_prototype(instance, proto);

        let readable_str = vm.alloc_string("readable".into());
        vm.set_property(instance, "__state__", readable_str);
        let queue = vm.alloc_array(Vec::new());
        vm.set_property(instance, "__queue__", queue);
        vm.set_property(instance, "__reader__", JsValue::undefined());
        vm.set_property(instance, "__stored_error__", JsValue::undefined());
        vm.set_property(instance, "__source__", underlying_source);

        // Controller
        let controller = vm.alloc_object();
        vm.set_property(controller, "__stream__", instance);

        let enqueue_fn = vm.alloc_native_function("enqueue", 1, |vm, this, args| {
            let chunk = args.first().copied().unwrap_or(JsValue::undefined());
            let stream = vm.get_property(this, "__stream__");
            let state = vm.get_property(stream, "__state__");
            let state_str = vm.to_string_value(state);
            if state_str != "readable" {
                return Err(VmError::TypeError(
                    "Cannot enqueue chunk into non-readable stream".into(),
                ));
            }

            let reader = vm.get_property(stream, "__reader__");
            if reader.is_js_object() {
                let pending_reads = vm.get_property(reader, "__pending_reads__");
                let shift_fn = vm.get_property(pending_reads, "shift");
                let next_read_prom = vm.call(shift_fn, pending_reads, &[])?;
                if !next_read_prom.is_undefined() {
                    let result_obj = vm.alloc_object();
                    vm.set_property(result_obj, "value", chunk);
                    vm.set_property(result_obj, "done", JsValue::from_boolean(false));
                    vm.resolve_promise(next_read_prom, result_obj);
                    return Ok(JsValue::undefined());
                }
            }

            let queue = vm.get_property(stream, "__queue__");
            let push_fn = vm.get_property(queue, "push");
            vm.call(push_fn, queue, &[chunk])?;
            Ok(JsValue::undefined())
        });
        vm.set_property(controller, "enqueue", enqueue_fn);

        let close_fn = vm.alloc_native_function("close", 0, |vm, this, _args| {
            let stream = vm.get_property(this, "__stream__");
            let state = vm.get_property(stream, "__state__");
            let state_str = vm.to_string_value(state);
            if state_str != "readable" {
                return Err(VmError::TypeError(
                    "Cannot close stream: stream is not in readable state".into(),
                ));
            }

            let closed_str = vm.alloc_string("closed".into());
            vm.set_property(stream, "__state__", closed_str);

            let reader = vm.get_property(stream, "__reader__");
            if reader.is_js_object() {
                let pending_reads = vm.get_property(reader, "__pending_reads__");
                let shift_fn = vm.get_property(pending_reads, "shift");
                loop {
                    let next_read = vm.call(shift_fn, pending_reads, &[])?;
                    if next_read.is_undefined() {
                        break;
                    }
                    let result_obj = vm.alloc_object();
                    vm.set_property(result_obj, "value", JsValue::undefined());
                    vm.set_property(result_obj, "done", JsValue::from_boolean(true));
                    vm.resolve_promise(next_read, result_obj);
                }

                let closed_prom = vm.get_property(reader, "__closed_promise__");
                vm.resolve_promise(closed_prom, JsValue::undefined());
            }

            Ok(JsValue::undefined())
        });
        vm.set_property(controller, "close", close_fn);

        let error_fn = vm.alloc_native_function("error", 1, |vm, this, args| {
            let err = args.first().copied().unwrap_or(JsValue::undefined());
            let stream = vm.get_property(this, "__stream__");

            let errored_str = vm.alloc_string("errored".into());
            vm.set_property(stream, "__state__", errored_str);
            vm.set_property(stream, "__stored_error__", err);

            let reader = vm.get_property(stream, "__reader__");
            if reader.is_js_object() {
                let pending_reads = vm.get_property(reader, "__pending_reads__");
                let shift_fn = vm.get_property(pending_reads, "shift");
                loop {
                    let next_read = vm.call(shift_fn, pending_reads, &[])?;
                    if next_read.is_undefined() {
                        break;
                    }
                    vm.reject_promise(next_read, err);
                }

                let closed_prom = vm.get_property(reader, "__closed_promise__");
                vm.reject_promise(closed_prom, err);
            }

            Ok(JsValue::undefined())
        });
        vm.set_property(controller, "error", error_fn);

        let desired_size_getter =
            vm.alloc_native_function("get desiredSize", 0, |vm, this, _args| {
                let stream = vm.get_property(this, "__stream__");
                let state = vm.get_property(stream, "__state__");
                let state_str = vm.to_string_value(state);
                if state_str == "errored" {
                    return Ok(JsValue::null());
                }
                if state_str == "closed" {
                    return Ok(JsValue::from_int32(0));
                }
                let queue = vm.get_property(stream, "__queue__");
                let queue_len_val = vm.get_property(queue, "length");
                let len = queue_len_val.to_int32().unwrap_or(0);
                let ds = if len == 0 { 1 } else { 0 };
                Ok(JsValue::from_int32(ds))
            });
        if let Some(ptr) = controller.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.define_builtin_accessor("desiredSize".to_string(), Some(desired_size_getter), None);
        }

        vm.set_property(instance, "__controller__", controller);

        // Call underlyingSource.start(controller)
        if underlying_source.is_js_object() {
            let start_fn = vm.get_property(underlying_source, "start");
            if vm.is_callable(start_fn) {
                let _ = vm.call(start_fn, underlying_source, &[controller]);
            }
        }

        Ok(instance)
    });
    vm.set_property(readable_ctor, "prototype", stream_proto);
    vm.globals.insert("ReadableStream".to_string(), readable_ctor);
}

fn register_writable_stream(vm: &mut Vm) {
    let writer_proto = vm.alloc_object();

    let writer_write = vm.alloc_native_function("write", 1, |vm, this, args| {
        let chunk = args.first().copied().unwrap_or(JsValue::undefined());
        let stream = vm.get_property(this, "__stream__");
        if !stream.is_js_object() {
            return Err(VmError::TypeError("Writer not attached to WritableStream".into()));
        }

        let state = vm.get_property(stream, "__state__");
        let state_str = vm.to_string_value(state);
        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

        if state_str == "errored" {
            let err = vm.get_property(stream, "__stored_error__");
            vm.reject_promise(promise, err);
            return Ok(promise);
        }
        if state_str == "closed" {
            let err = vm.alloc_string("Cannot write to closed WritableStream".into());
            vm.reject_promise(promise, err);
            return Ok(promise);
        }

        let sink = vm.get_property(stream, "__sink__");
        if sink.is_js_object() {
            let write_fn = vm.get_property(sink, "write");
            if vm.is_callable(write_fn) {
                let controller = vm.get_property(stream, "__controller__");
                let res = vm.call(write_fn, sink, &[chunk, controller])?;
                if res.is_js_object() && vm.get_property(res, "then").is_js_object() {
                    return Ok(res);
                }
            }
        }

        vm.resolve_promise(promise, JsValue::undefined());
        Ok(promise)
    });
    vm.set_property(writer_proto, "write", writer_write);

    let writer_close = vm.alloc_native_function("close", 0, |vm, this, _args| {
        let stream = vm.get_property(this, "__stream__");
        if !stream.is_js_object() {
            return Err(VmError::TypeError("Writer not attached to WritableStream".into()));
        }
        let close_fn = vm.get_property(stream, "close");
        vm.call(close_fn, stream, &[])
    });
    vm.set_property(writer_proto, "close", writer_close);

    let writer_abort = vm.alloc_native_function("abort", 1, |vm, this, args| {
        let stream = vm.get_property(this, "__stream__");
        if !stream.is_js_object() {
            return Err(VmError::TypeError("Writer not attached to WritableStream".into()));
        }
        let abort_fn = vm.get_property(stream, "abort");
        vm.call(abort_fn, stream, args)
    });
    vm.set_property(writer_proto, "abort", writer_abort);

    let writer_release_lock = vm.alloc_native_function("releaseLock", 0, |vm, this, _args| {
        let stream = vm.get_property(this, "__stream__");
        if stream.is_js_object() {
            vm.set_property(stream, "__writer__", JsValue::undefined());
            vm.set_property(this, "__stream__", JsValue::undefined());
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(writer_proto, "releaseLock", writer_release_lock);

    let writer_ready_getter = vm.alloc_native_function("get ready", 0, |vm, this, _args| {
        let ready_prom = vm.get_property(this, "__ready_promise__");
        Ok(ready_prom)
    });
    let writer_closed_getter = vm.alloc_native_function("get closed", 0, |vm, this, _args| {
        let closed_prom = vm.get_property(this, "__closed_promise__");
        Ok(closed_prom)
    });
    if let Some(ptr) = writer_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor("ready".to_string(), Some(writer_ready_getter), None);
        obj.define_builtin_accessor("closed".to_string(), Some(writer_closed_getter), None);
    }

    let writer_ctor =
        vm.alloc_native_function("WritableStreamDefaultWriter", 1, |vm, _this, args| {
            let stream = args.first().copied().unwrap_or(JsValue::undefined());
            if !stream.is_js_object() {
                return Err(VmError::TypeError(
                    "WritableStreamDefaultWriter constructor requires a WritableStream".into(),
                ));
            }

            let existing = vm.get_property(stream, "__writer__");
            if !existing.is_undefined() {
                return Err(VmError::TypeError(
                    "WritableStream is already locked by another writer".into(),
                ));
            }

            let ctor = vm
                .globals
                .get("WritableStreamDefaultWriter")
                .copied()
                .unwrap_or(JsValue::undefined());
            let proto = vm.get_property(ctor, "prototype");

            let instance = vm.alloc_object();
            vm.set_prototype(instance, proto);

            vm.set_property(instance, "__stream__", stream);
            let ready = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            vm.resolve_promise(ready, JsValue::undefined());
            vm.set_property(instance, "__ready_promise__", ready);

            let closed = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            vm.set_property(instance, "__closed_promise__", closed);

            vm.set_property(stream, "__writer__", instance);
            Ok(instance)
        });
    vm.set_property(writer_ctor, "prototype", writer_proto);
    vm.globals.insert("WritableStreamDefaultWriter".to_string(), writer_ctor);

    // WritableStream prototype
    let stream_proto = vm.alloc_object();

    let stream_locked_getter = vm.alloc_native_function("get locked", 0, |vm, this, _args| {
        let writer = vm.get_property(this, "__writer__");
        Ok(JsValue::from_boolean(!writer.is_undefined()))
    });
    if let Some(ptr) = stream_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor("locked".to_string(), Some(stream_locked_getter), None);
    }

    let stream_get_writer = vm.alloc_native_function("getWriter", 0, |vm, this, _args| {
        let ctor =
            vm.globals.get("WritableStreamDefaultWriter").copied().unwrap_or(JsValue::undefined());
        vm.call(ctor, JsValue::undefined(), &[this])
    });
    vm.set_property(stream_proto, "getWriter", stream_get_writer);

    let stream_close = vm.alloc_native_function("close", 0, |vm, this, _args| {
        let closed_str = vm.alloc_string("closed".into());
        vm.set_property(this, "__state__", closed_str);

        let sink = vm.get_property(this, "__sink__");
        if sink.is_js_object() {
            let close_fn = vm.get_property(sink, "close");
            if vm.is_callable(close_fn) {
                let _ = vm.call(close_fn, sink, &[]);
            }
        }

        let writer = vm.get_property(this, "__writer__");
        if writer.is_js_object() {
            let closed_prom = vm.get_property(writer, "__closed_promise__");
            vm.resolve_promise(closed_prom, JsValue::undefined());
        }

        let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
        vm.resolve_promise(p, JsValue::undefined());
        Ok(p)
    });
    vm.set_property(stream_proto, "close", stream_close);

    let stream_abort = vm.alloc_native_function("abort", 1, |vm, this, args| {
        let reason = args.first().copied().unwrap_or(JsValue::undefined());
        let errored_str = vm.alloc_string("errored".into());
        vm.set_property(this, "__state__", errored_str);
        vm.set_property(this, "__stored_error__", reason);

        let sink = vm.get_property(this, "__sink__");
        if sink.is_js_object() {
            let abort_fn = vm.get_property(sink, "abort");
            if vm.is_callable(abort_fn) {
                let _ = vm.call(abort_fn, sink, &[reason]);
            }
        }

        let writer = vm.get_property(this, "__writer__");
        if writer.is_js_object() {
            let closed_prom = vm.get_property(writer, "__closed_promise__");
            vm.reject_promise(closed_prom, reason);
        }

        let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
        vm.resolve_promise(p, reason);
        Ok(p)
    });
    vm.set_property(stream_proto, "abort", stream_abort);

    let writable_ctor = vm.alloc_native_function("WritableStream", 2, |vm, _this, args| {
        let underlying_sink = args.first().copied().unwrap_or(JsValue::undefined());
        let _strategy = args.get(1).copied().unwrap_or(JsValue::undefined());

        let ctor = vm.globals.get("WritableStream").copied().unwrap_or(JsValue::undefined());
        let proto = vm.get_property(ctor, "prototype");

        let instance = vm.alloc_object();
        vm.set_prototype(instance, proto);

        let writable_str = vm.alloc_string("writable".into());
        vm.set_property(instance, "__state__", writable_str);
        vm.set_property(instance, "__writer__", JsValue::undefined());
        vm.set_property(instance, "__stored_error__", JsValue::undefined());
        vm.set_property(instance, "__sink__", underlying_sink);

        let controller = vm.alloc_object();
        vm.set_property(controller, "__stream__", instance);
        let error_fn = vm.alloc_native_function("error", 1, |vm, this, args| {
            let err = args.first().copied().unwrap_or(JsValue::undefined());
            let stream = vm.get_property(this, "__stream__");
            let errored_str = vm.alloc_string("errored".into());
            vm.set_property(stream, "__state__", errored_str);
            vm.set_property(stream, "__stored_error__", err);
            Ok(JsValue::undefined())
        });
        vm.set_property(controller, "error", error_fn);
        vm.set_property(instance, "__controller__", controller);

        if underlying_sink.is_js_object() {
            let start_fn = vm.get_property(underlying_sink, "start");
            if vm.is_callable(start_fn) {
                let _ = vm.call(start_fn, underlying_sink, &[controller]);
            }
        }

        Ok(instance)
    });
    vm.set_property(writable_ctor, "prototype", stream_proto);
    vm.globals.insert("WritableStream".to_string(), writable_ctor);
}

fn register_transform_stream(vm: &mut Vm) {
    let transform_script = r#"
        (function(ReadableStream, WritableStream) {
            function TransformStream(transformer, writableStrategy, readableStrategy) {
                transformer = transformer || {};
                var self = this;
                var readableController;
                var writableController;

                this.readable = new ReadableStream({
                    start: function(controller) {
                        readableController = controller;
                    }
                }, readableStrategy);

                var transformController = {
                    enqueue: function(chunk) {
                        readableController.enqueue(chunk);
                    },
                    error: function(reason) {
                        readableController.error(reason);
                        writableController.error(reason);
                    },
                    terminate: function() {
                        readableController.close();
                    },
                    get desiredSize() {
                        return readableController.desiredSize;
                    }
                };

                this.writable = new WritableStream({
                    start: function(controller) {
                        writableController = controller;
                        if (transformer.start) {
                            return transformer.start(transformController);
                        }
                    },
                    write: function(chunk, controller) {
                        if (transformer.transform) {
                            return transformer.transform(chunk, transformController);
                        }
                        readableController.enqueue(chunk);
                    },
                    close: function() {
                        if (transformer.flush) {
                            return Promise.resolve(transformer.flush(transformController)).then(function() {
                                readableController.close();
                            });
                        }
                        readableController.close();
                    },
                    abort: function(reason) {
                        readableController.error(reason);
                    }
                }, writableStrategy);
            }
            return TransformStream;
        })
    "#;
    let make_transform_fn = vm.eval(transform_script).expect("compile TransformStream constructor");
    let rs_ctor = vm.globals.get("ReadableStream").copied().unwrap_or(JsValue::undefined());
    let ws_ctor = vm.globals.get("WritableStream").copied().unwrap_or(JsValue::undefined());
    let transform_ctor = vm
        .call(make_transform_fn, JsValue::undefined(), &[rs_ctor, ws_ctor])
        .expect("init TransformStream");
    vm.globals.insert("TransformStream".to_string(), transform_ctor);
}

fn register_queuing_strategies(vm: &mut Vm) {
    let strategy_script = r#"
        (function() {
            function ByteLengthQueuingStrategy(options) {
                this.highWaterMark = options && options.highWaterMark !== undefined ? options.highWaterMark : 0;
            }
            ByteLengthQueuingStrategy.prototype.size = function(chunk) {
                return chunk && chunk.byteLength !== undefined ? chunk.byteLength : 0;
            };

            function CountQueuingStrategy(options) {
                this.highWaterMark = options && options.highWaterMark !== undefined ? options.highWaterMark : 0;
            }
            CountQueuingStrategy.prototype.size = function(chunk) {
                return 1;
            };

            return {
                ByteLengthQueuingStrategy: ByteLengthQueuingStrategy,
                CountQueuingStrategy: CountQueuingStrategy
            };
        })()
    "#;
    if let Ok(res) = vm.eval(strategy_script) {
        let blqs = vm.get_property(res, "ByteLengthQueuingStrategy");
        let cqs = vm.get_property(res, "CountQueuingStrategy");
        vm.globals.insert("ByteLengthQueuingStrategy".to_string(), blqs);
        vm.globals.insert("CountQueuingStrategy".to_string(), cqs);
    }
}
