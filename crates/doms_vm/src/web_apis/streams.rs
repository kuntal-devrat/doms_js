//! WHATWG Streams Standard (`ReadableStream`, `WritableStream`, `TransformStream`).

use super::*;
use std::sync::Arc;

pub(super) fn register_streams(vm: &mut Vm) {
    register_byte_stream_controller_and_byob(vm);
    register_readable_stream(vm);
    register_writable_stream(vm);
    register_transform_stream(vm);
    register_queuing_strategies(vm);
}

fn is_array_buffer_view(val: JsValue) -> bool {
    if let Some(ptr) = val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        matches!(obj.kind, ObjectKind::TypedArray(_) | ObjectKind::DataView { .. })
    } else {
        false
    }
}

fn get_view_byte_length(val: JsValue) -> usize {
    if let Some(ptr) = val.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        match &obj.kind {
            ObjectKind::TypedArray(ta) => ta.length * ta.kind.bytes_per_element(),
            ObjectKind::DataView { byte_length, .. } => *byte_length,
            _ => 0,
        }
    } else {
        0
    }
}

fn copy_view_bytes(
    dst: JsValue,
    dst_byte_offset: usize,
    src: JsValue,
    src_byte_offset: usize,
    count: usize,
) -> usize {
    let (src_buf, src_start) = match src.as_object_ptr() {
        Some(ptr) => {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::TypedArray(ta) => (Arc::clone(&ta.buffer), ta.byte_offset),
                ObjectKind::DataView { buffer, byte_offset, .. } => {
                    (Arc::clone(buffer), *byte_offset)
                }
                _ => return 0,
            }
        }
        None => return 0,
    };
    let (dst_buf, dst_start) = match dst.as_object_ptr() {
        Some(ptr) => {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::TypedArray(ta) => (Arc::clone(&ta.buffer), ta.byte_offset),
                ObjectKind::DataView { buffer, byte_offset, .. } => {
                    (Arc::clone(buffer), *byte_offset)
                }
                _ => return 0,
            }
        }
        None => return 0,
    };
    let mut copied = 0;
    for i in 0..count {
        if let Some(b) = src_buf.get(src_start + src_byte_offset + i) {
            if dst_buf.set(dst_start + dst_byte_offset + i, b) {
                copied += 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    copied
}

fn error_writable_stream_and_writer(vm: &mut Vm, stream: JsValue, writer: JsValue, err: JsValue) {
    let errored_str = vm.alloc_string("errored".into());
    vm.set_property(stream, "__state__", errored_str);
    vm.set_property(stream, "__stored_error__", err);

    if writer.is_js_object() {
        let closed_prom = vm.get_property(writer, "__closed_promise__");
        vm.reject_promise(closed_prom, err);

        let rejected_ready = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
        vm.reject_promise(rejected_ready, err);
        vm.set_property(writer, "__ready_promise__", rejected_ready);

        let pending_writes = vm.get_property(writer, "__pending_writes__");
        if pending_writes.is_js_object() {
            let shift_fn = vm.get_property(pending_writes, "shift");
            loop {
                let next_p = match vm.call(shift_fn, pending_writes, &[]) {
                    Ok(p) => p,
                    Err(_) => break,
                };
                if next_p.is_undefined() {
                    break;
                }
                vm.reject_promise(next_p, err);
            }
        }
    }
}

fn register_byte_stream_controller_and_byob(vm: &mut Vm) {
    // 1. ReadableStreamBYOBRequest prototype & constructor
    let byob_req_proto = vm.alloc_object();

    let byob_req_view_getter = vm.alloc_native_function("get view", 0, |vm, this, _args| {
        let view = vm.get_property(this, "__view__");
        Ok(view)
    });
    if let Some(ptr) = byob_req_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor("view".to_string(), Some(byob_req_view_getter), None);
    }

    let byob_req_respond = vm.alloc_native_function("respond", 1, |vm, this, args| {
        let controller = vm.get_property(this, "__controller__");
        if !controller.is_js_object() {
            return Err(VmError::TypeError("Invalid ReadableStreamBYOBRequest".into()));
        }
        let stream = vm.get_property(controller, "__stream__");
        let view = vm.get_property(this, "__view__");
        if view.is_undefined() {
            return Err(VmError::TypeError("BYOB request already responded".into()));
        }
        let bytes_written = args.first().and_then(|v| v.to_int32()).unwrap_or(0) as usize;
        let view_len = get_view_byte_length(view);
        if bytes_written > view_len {
            return Err(VmError::RangeError("bytesWritten exceeds view byteLength".into()));
        }

        vm.set_property(this, "__view__", JsValue::undefined());
        vm.set_property(controller, "__byob_request__", JsValue::null());

        let reader = vm.get_property(stream, "__reader__");
        if reader.is_js_object() {
            let pending_reads = vm.get_property(reader, "__pending_reads__");
            let shift_fn = vm.get_property(pending_reads, "shift");
            let req_item = vm.call(shift_fn, pending_reads, &[])?;
            if req_item.is_js_object() {
                let promise = vm.get_property(req_item, "promise");
                let state_val = vm.get_property(stream, "__state__");
                let state_str = vm.to_string_value(state_val);
                let done = state_str == "closed" && bytes_written == 0;
                let result_obj = vm.alloc_object();
                vm.set_property(result_obj, "value", view);
                vm.set_property(result_obj, "done", JsValue::from_boolean(done));
                vm.resolve_promise(promise, result_obj);
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(byob_req_proto, "respond", byob_req_respond);

    let byob_req_respond_with_new_view =
        vm.alloc_native_function("respondWithNewView", 1, |vm, this, args| {
            let view = args.first().copied().unwrap_or(JsValue::undefined());
            if !is_array_buffer_view(view) {
                return Err(VmError::TypeError("respondWithNewView requires an ArrayBufferView".into()));
            }
            let controller = vm.get_property(this, "__controller__");
            if !controller.is_js_object() {
                return Err(VmError::TypeError("Invalid ReadableStreamBYOBRequest".into()));
            }
            let stream = vm.get_property(controller, "__stream__");
            vm.set_property(this, "__view__", JsValue::undefined());
            vm.set_property(controller, "__byob_request__", JsValue::null());

            let reader = vm.get_property(stream, "__reader__");
            if reader.is_js_object() {
                let pending_reads = vm.get_property(reader, "__pending_reads__");
                let shift_fn = vm.get_property(pending_reads, "shift");
                let req_item = vm.call(shift_fn, pending_reads, &[])?;
                if req_item.is_js_object() {
                    let promise = vm.get_property(req_item, "promise");
                    let result_obj = vm.alloc_object();
                    vm.set_property(result_obj, "value", view);
                    vm.set_property(result_obj, "done", JsValue::from_boolean(false));
                    vm.resolve_promise(promise, result_obj);
                }
            }
            Ok(JsValue::undefined())
        });
    vm.set_property(byob_req_proto, "respondWithNewView", byob_req_respond_with_new_view);

    let byob_req_ctor =
        vm.alloc_native_function("ReadableStreamBYOBRequest", 0, |_vm, _this, _args| {
            Err(VmError::TypeError(
                "ReadableStreamBYOBRequest cannot be directly constructed".into(),
            ))
        });
    vm.set_property(byob_req_ctor, "prototype", byob_req_proto);
    vm.globals.insert("ReadableStreamBYOBRequest".to_string(), byob_req_ctor);

    // 2. ReadableByteStreamController prototype & constructor
    let byte_ctrl_proto = vm.alloc_object();

    let byte_ctrl_byob_req_getter =
        vm.alloc_native_function("get byobRequest", 0, |vm, this, _args| {
            let req = vm.get_property(this, "__byob_request__");
            Ok(req)
        });
    let byte_ctrl_desired_size_getter =
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
    if let Some(ptr) = byte_ctrl_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor(
            "byobRequest".to_string(),
            Some(byte_ctrl_byob_req_getter),
            None,
        );
        obj.define_builtin_accessor(
            "desiredSize".to_string(),
            Some(byte_ctrl_desired_size_getter),
            None,
        );
    }

    let byte_ctrl_close = vm.alloc_native_function("close", 0, |vm, this, _args| {
        let stream = vm.get_property(this, "__stream__");
        let state = vm.get_property(stream, "__state__");
        let state_str = vm.to_string_value(state);
        if state_str != "readable" {
            return Err(VmError::TypeError("Stream is not readable".into()));
        }
        let closed_str = vm.alloc_string("closed".into());
        vm.set_property(stream, "__state__", closed_str);

        let byob_req = vm.get_property(this, "__byob_request__");
        if byob_req.is_js_object() {
            let view = vm.get_property(byob_req, "__view__");
            vm.set_property(byob_req, "__view__", JsValue::undefined());
            vm.set_property(this, "__byob_request__", JsValue::null());

            let reader = vm.get_property(stream, "__reader__");
            if reader.is_js_object() {
                let pending_reads = vm.get_property(reader, "__pending_reads__");
                let shift_fn = vm.get_property(pending_reads, "shift");
                let req_item = vm.call(shift_fn, pending_reads, &[])?;
                if req_item.is_js_object() {
                    let promise = vm.get_property(req_item, "promise");
                    let result_obj = vm.alloc_object();
                    vm.set_property(result_obj, "value", view);
                    vm.set_property(result_obj, "done", JsValue::from_boolean(true));
                    vm.resolve_promise(promise, result_obj);
                }
                let closed_prom = vm.get_property(reader, "__closed_promise__");
                vm.resolve_promise(closed_prom, JsValue::undefined());
            }
        } else {
            let reader = vm.get_property(stream, "__reader__");
            if reader.is_js_object() {
                let pending_reads = vm.get_property(reader, "__pending_reads__");
                let shift_fn = vm.get_property(pending_reads, "shift");
                loop {
                    let next_read = vm.call(shift_fn, pending_reads, &[])?;
                    if next_read.is_undefined() {
                        break;
                    }
                    let prom = if next_read.is_js_object()
                        && !vm.get_property(next_read, "promise").is_undefined()
                    {
                        vm.get_property(next_read, "promise")
                    } else {
                        next_read
                    };
                    let result_obj = vm.alloc_object();
                    vm.set_property(result_obj, "value", JsValue::undefined());
                    vm.set_property(result_obj, "done", JsValue::from_boolean(true));
                    vm.resolve_promise(prom, result_obj);
                }
                let closed_prom = vm.get_property(reader, "__closed_promise__");
                vm.resolve_promise(closed_prom, JsValue::undefined());
            }
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(byte_ctrl_proto, "close", byte_ctrl_close);

    let byte_ctrl_enqueue = vm.alloc_native_function("enqueue", 1, |vm, this, args| {
        let chunk = args.first().copied().unwrap_or(JsValue::undefined());
        if !is_array_buffer_view(chunk) {
            return Err(VmError::TypeError("chunk must be an ArrayBufferView".into()));
        }
        let stream = vm.get_property(this, "__stream__");
        let state = vm.get_property(stream, "__state__");
        let state_str = vm.to_string_value(state);
        if state_str != "readable" {
            return Err(VmError::TypeError(
                "Cannot enqueue chunk into non-readable stream".into(),
            ));
        }

        let byob_req = vm.get_property(this, "__byob_request__");
        if byob_req.is_js_object() {
            let dst_view = vm.get_property(byob_req, "__view__");
            if is_array_buffer_view(dst_view) {
                let dst_len = get_view_byte_length(dst_view);
                let chunk_len = get_view_byte_length(chunk);
                let to_copy = dst_len.min(chunk_len);
                copy_view_bytes(dst_view, 0, chunk, 0, to_copy);

                let respond_fn = vm.get_property(byob_req, "respond");
                vm.call(respond_fn, byob_req, &[JsValue::from_int32(to_copy as i32)])?;

                if chunk_len > to_copy {
                    let sub_fn = vm.get_property(chunk, "subarray");
                    if vm.is_callable(sub_fn) {
                        let rem =
                            vm.call(sub_fn, chunk, &[JsValue::from_int32(to_copy as i32)])?;
                        let queue = vm.get_property(stream, "__queue__");
                        let push_fn = vm.get_property(queue, "push");
                        vm.call(push_fn, queue, &[rem])?;
                    }
                }
                return Ok(JsValue::undefined());
            }
        }

        let reader = vm.get_property(stream, "__reader__");
        if reader.is_js_object() {
            let pending_reads = vm.get_property(reader, "__pending_reads__");
            let shift_fn = vm.get_property(pending_reads, "shift");
            let next_read_item = vm.call(shift_fn, pending_reads, &[])?;
            if !next_read_item.is_undefined() {
                if next_read_item.is_js_object()
                    && !vm.get_property(next_read_item, "promise").is_undefined()
                {
                    let dst_view = vm.get_property(next_read_item, "view");
                    if is_array_buffer_view(dst_view) {
                        let dst_len = get_view_byte_length(dst_view);
                        let chunk_len = get_view_byte_length(chunk);
                        let to_copy = dst_len.min(chunk_len);
                        copy_view_bytes(dst_view, 0, chunk, 0, to_copy);
                        let res_obj = vm.alloc_object();
                        vm.set_property(res_obj, "value", dst_view);
                        vm.set_property(res_obj, "done", JsValue::from_boolean(false));
                        let p = vm.get_property(next_read_item, "promise");
                        vm.resolve_promise(p, res_obj);
                        if chunk_len > to_copy {
                            let sub_fn = vm.get_property(chunk, "subarray");
                            if vm.is_callable(sub_fn) {
                                let rem = vm
                                    .call(sub_fn, chunk, &[JsValue::from_int32(to_copy as i32)])?;
                                let queue = vm.get_property(stream, "__queue__");
                                let push_fn = vm.get_property(queue, "push");
                                vm.call(push_fn, queue, &[rem])?;
                            }
                        }
                        return Ok(JsValue::undefined());
                    }
                }

                let prom = if next_read_item.is_js_object()
                    && !vm.get_property(next_read_item, "promise").is_undefined()
                {
                    vm.get_property(next_read_item, "promise")
                } else {
                    next_read_item
                };
                let result_obj = vm.alloc_object();
                vm.set_property(result_obj, "value", chunk);
                vm.set_property(result_obj, "done", JsValue::from_boolean(false));
                vm.resolve_promise(prom, result_obj);
                return Ok(JsValue::undefined());
            }
        }

        let queue = vm.get_property(stream, "__queue__");
        let push_fn = vm.get_property(queue, "push");
        vm.call(push_fn, queue, &[chunk])?;
        Ok(JsValue::undefined())
    });
    vm.set_property(byte_ctrl_proto, "enqueue", byte_ctrl_enqueue);

    let byte_ctrl_error = vm.alloc_native_function("error", 1, |vm, this, args| {
        let err = args.first().copied().unwrap_or(JsValue::undefined());
        let stream = vm.get_property(this, "__stream__");
        let errored_str = vm.alloc_string("errored".into());
        vm.set_property(stream, "__state__", errored_str);
        vm.set_property(stream, "__stored_error__", err);
        vm.set_property(this, "__byob_request__", JsValue::null());

        let reader = vm.get_property(stream, "__reader__");
        if reader.is_js_object() {
            let pending_reads = vm.get_property(reader, "__pending_reads__");
            let shift_fn = vm.get_property(pending_reads, "shift");
            loop {
                let next_read = vm.call(shift_fn, pending_reads, &[])?;
                if next_read.is_undefined() {
                    break;
                }
                let prom = if next_read.is_js_object()
                    && !vm.get_property(next_read, "promise").is_undefined()
                {
                    vm.get_property(next_read, "promise")
                } else {
                    next_read
                };
                vm.reject_promise(prom, err);
            }
            let closed_prom = vm.get_property(reader, "__closed_promise__");
            vm.reject_promise(closed_prom, err);
        }
        Ok(JsValue::undefined())
    });
    vm.set_property(byte_ctrl_proto, "error", byte_ctrl_error);

    let byte_ctrl_ctor =
        vm.alloc_native_function("ReadableByteStreamController", 0, |_vm, _this, _args| {
            Err(VmError::TypeError(
                "ReadableByteStreamController cannot be directly constructed".into(),
            ))
        });
    vm.set_property(byte_ctrl_ctor, "prototype", byte_ctrl_proto);
    vm.globals.insert("ReadableByteStreamController".to_string(), byte_ctrl_ctor);

    // 3. ReadableStreamBYOBReader prototype & constructor
    let byob_reader_proto = vm.alloc_object();

    let byob_reader_read = vm.alloc_native_function("read", 1, |vm, this, args| {
        let view = args.first().copied().unwrap_or(JsValue::undefined());
        if !is_array_buffer_view(view) {
            let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            let err = vm.alloc_string("read(view) requires an ArrayBufferView".into());
            vm.reject_promise(p, err);
            return Ok(p);
        }
        let view_len = get_view_byte_length(view);
        if view_len == 0 {
            let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            let err = vm.alloc_string("view byteLength must be greater than 0".into());
            vm.reject_promise(p, err);
            return Ok(p);
        }
        let stream = vm.get_property(this, "__stream__");
        if !stream.is_js_object() {
            let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            let err = vm.alloc_string("Reader not attached to stream".into());
            vm.reject_promise(p, err);
            return Ok(p);
        }

        let state_val = vm.get_property(stream, "__state__");
        let state_str = vm.to_string_value(state_val);
        if state_str == "errored" {
            let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            let err = vm.get_property(stream, "__stored_error__");
            vm.reject_promise(p, err);
            return Ok(p);
        }
        if state_str == "closed" {
            let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            let res = vm.alloc_object();
            vm.set_property(res, "value", view);
            vm.set_property(res, "done", JsValue::from_boolean(true));
            vm.resolve_promise(p, res);
            return Ok(p);
        }

        let queue = vm.get_property(stream, "__queue__");
        let queue_len_val = vm.get_property(queue, "length");
        let queue_len = queue_len_val.to_int32().unwrap_or(0);
        if queue_len > 0 {
            let shift_fn = vm.get_property(queue, "shift");
            let chunk = vm.call(shift_fn, queue, &[])?;
            if is_array_buffer_view(chunk) {
                let chunk_len = get_view_byte_length(chunk);
                let to_copy = view_len.min(chunk_len);
                copy_view_bytes(view, 0, chunk, 0, to_copy);
                if chunk_len > to_copy {
                    let sub_fn = vm.get_property(chunk, "subarray");
                    if vm.is_callable(sub_fn) {
                        let rem =
                            vm.call(sub_fn, chunk, &[JsValue::from_int32(to_copy as i32)])?;
                        let unshift_fn = vm.get_property(queue, "unshift");
                        vm.call(unshift_fn, queue, &[rem])?;
                    }
                }
            }
            let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            let res = vm.alloc_object();
            vm.set_property(res, "value", view);
            vm.set_property(res, "done", JsValue::from_boolean(false));
            vm.resolve_promise(p, res);
            return Ok(p);
        }

        let promise = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
        let req_item = vm.alloc_object();
        vm.set_property(req_item, "promise", promise);
        vm.set_property(req_item, "view", view);
        let pending = vm.get_property(this, "__pending_reads__");
        let push_fn = vm.get_property(pending, "push");
        vm.call(push_fn, pending, &[req_item])?;

        let controller = vm.get_property(stream, "__controller__");
        let byob_ctor =
            vm.globals.get("ReadableStreamBYOBRequest").copied().unwrap_or(JsValue::undefined());
        let byob_proto = vm.get_property(byob_ctor, "prototype");
        let byob_req = vm.alloc_object();
        vm.set_prototype(byob_req, byob_proto);
        vm.set_property(byob_req, "__controller__", controller);
        vm.set_property(byob_req, "__view__", view);
        vm.set_property(controller, "__byob_request__", byob_req);

        let source = vm.get_property(stream, "__source__");
        if source.is_js_object() {
            let pull_fn = vm.get_property(source, "pull");
            if vm.is_callable(pull_fn) {
                let _ = vm.call(pull_fn, source, &[controller]);
            }
        }
        Ok(promise)
    });
    vm.set_property(byob_reader_proto, "read", byob_reader_read);

    let byob_reader_release_lock =
        vm.alloc_native_function("releaseLock", 0, |vm, this, _args| {
            let stream = vm.get_property(this, "__stream__");
            if stream.is_js_object() {
                vm.set_property(stream, "__reader__", JsValue::undefined());
                vm.set_property(this, "__stream__", JsValue::undefined());
            }
            Ok(JsValue::undefined())
        });
    vm.set_property(byob_reader_proto, "releaseLock", byob_reader_release_lock);

    let byob_reader_cancel = vm.alloc_native_function("cancel", 1, |vm, this, args| {
        let stream = vm.get_property(this, "__stream__");
        if !stream.is_js_object() {
            return Err(VmError::TypeError("Reader not attached to stream".into()));
        }
        let cancel_fn = vm.get_property(stream, "cancel");
        vm.call(cancel_fn, stream, args)
    });
    vm.set_property(byob_reader_proto, "cancel", byob_reader_cancel);

    let byob_reader_closed_getter =
        vm.alloc_native_function("get closed", 0, |vm, this, _args| {
            let closed_prom = vm.get_property(this, "__closed_promise__");
            Ok(closed_prom)
        });
    if let Some(ptr) = byob_reader_proto.as_object_ptr() {
        let obj = unsafe { &mut *(ptr as *mut JsObject) };
        obj.define_builtin_accessor(
            "closed".to_string(),
            Some(byob_reader_closed_getter),
            None,
        );
    }

    let byob_reader_ctor =
        vm.alloc_native_function("ReadableStreamBYOBReader", 1, |vm, _this, args| {
            let stream = args.first().copied().unwrap_or(JsValue::undefined());
            if !stream.is_js_object() {
                return Err(VmError::TypeError(
                    "ReadableStreamBYOBReader constructor requires a ReadableStream".into(),
                ));
            }
            let is_byte = vm.get_property(stream, "__is_byte_stream__");
            if !is_byte.is_truthy() {
                return Err(VmError::TypeError(
                    "Cannot construct ReadableStreamBYOBReader on non-byte stream".into(),
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
                .get("ReadableStreamBYOBReader")
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
    vm.set_property(byob_reader_ctor, "prototype", byob_reader_proto);
    vm.globals.insert("ReadableStreamBYOBReader".to_string(), byob_reader_ctor);
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

    let stream_get_reader = vm.alloc_native_function("getReader", 1, |vm, this, args| {
        let options = args.first().copied().unwrap_or(JsValue::undefined());
        let mut is_byob = false;
        if options.is_js_object() {
            let mode = vm.get_property(options, "mode");
            if vm.to_string_value(mode) == "byob" {
                is_byob = true;
            }
        }
        if is_byob {
            let is_byte = vm.get_property(this, "__is_byte_stream__");
            if !is_byte.is_truthy() {
                return Err(VmError::TypeError(
                    "getReader with 'byob' mode requires a byte stream".into(),
                ));
            }
            let ctor = vm
                .globals
                .get("ReadableStreamBYOBReader")
                .copied()
                .unwrap_or(JsValue::undefined());
            vm.call(ctor, JsValue::undefined(), &[this])
        } else {
            let ctor = vm
                .globals
                .get("ReadableStreamDefaultReader")
                .copied()
                .unwrap_or(JsValue::undefined());
            vm.call(ctor, JsValue::undefined(), &[this])
        }
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

        let is_byte_stream = if underlying_source.is_js_object() {
            let t = vm.get_property(underlying_source, "type");
            vm.to_string_value(t) == "bytes"
        } else {
            false
        };
        vm.set_property(instance, "__is_byte_stream__", JsValue::from_boolean(is_byte_stream));

        if is_byte_stream {
            let ctrl_ctor = vm
                .globals
                .get("ReadableByteStreamController")
                .copied()
                .unwrap_or(JsValue::undefined());
            let ctrl_proto = vm.get_property(ctrl_ctor, "prototype");
            let controller = vm.alloc_object();
            vm.set_prototype(controller, ctrl_proto);
            vm.set_property(controller, "__stream__", instance);
            vm.set_property(controller, "__byob_request__", JsValue::null());
            vm.set_property(instance, "__controller__", controller);

            if underlying_source.is_js_object() {
                let start_fn = vm.get_property(underlying_source, "start");
                if vm.is_callable(start_fn) {
                    let _ = vm.call(start_fn, underlying_source, &[controller]);
                }
            }
        } else {
            // Standard Controller
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
                obj.define_builtin_accessor(
                    "desiredSize".to_string(),
                    Some(desired_size_getter),
                    None,
                );
            }

            vm.set_property(instance, "__controller__", controller);

            if underlying_source.is_js_object() {
                let start_fn = vm.get_property(underlying_source, "start");
                if vm.is_callable(start_fn) {
                    let _ = vm.call(start_fn, underlying_source, &[controller]);
                }
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

        let pending_writes = vm.get_property(this, "__pending_writes__");
        if pending_writes.is_js_object() {
            let push_fn = vm.get_property(pending_writes, "push");
            let _ = vm.call(push_fn, pending_writes, &[promise]);
        }

        let sink = vm.get_property(stream, "__sink__");
        if sink.is_js_object() {
            let write_fn = vm.get_property(sink, "write");
            if vm.is_callable(write_fn) {
                let controller = vm.get_property(stream, "__controller__");
                match vm.call(write_fn, sink, &[chunk, controller]) {
                    Ok(res) => {
                        if res.is_js_object() {
                            let then_fn = vm.get_property(res, "then");
                            if vm.is_callable(then_fn) {
                                let p_clone = promise;
                                let stream_val = stream;
                                let writer_val = this;

                                let on_fulfilled = vm.alloc_closure_native_function(
                                    "onFulfilled",
                                    0,
                                    Arc::new(move |vm, _t, _a| {
                                        vm.resolve_promise(p_clone, JsValue::undefined());
                                        Ok(JsValue::undefined())
                                    }),
                                );
                                let on_rejected = vm.alloc_closure_native_function(
                                    "onRejected",
                                    1,
                                    Arc::new(move |vm, _t, r_args| {
                                        let err =
                                            r_args.first().copied().unwrap_or(JsValue::undefined());
                                        error_writable_stream_and_writer(
                                            vm, stream_val, writer_val, err,
                                        );
                                        vm.reject_promise(p_clone, err);
                                        Ok(JsValue::undefined())
                                    }),
                                );
                                let _ = vm.call(then_fn, res, &[on_fulfilled, on_rejected]);
                                return Ok(promise);
                            }
                        }
                    }
                    Err(err) => {
                        let err_val = match err {
                            VmError::Thrown(v) => v,
                            e => vm.alloc_string(e.to_string()),
                        };
                        error_writable_stream_and_writer(vm, stream, this, err_val);
                        vm.reject_promise(promise, err_val);
                        return Ok(promise);
                    }
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
        let reason = args.first().copied().unwrap_or(JsValue::undefined());
        let stream = vm.get_property(this, "__stream__");
        if !stream.is_js_object() {
            return Err(VmError::TypeError("Writer not attached to WritableStream".into()));
        }

        error_writable_stream_and_writer(vm, stream, this, reason);

        let sink = vm.get_property(stream, "__sink__");
        if sink.is_js_object() {
            let abort_fn = vm.get_property(sink, "abort");
            if vm.is_callable(abort_fn) {
                let controller = vm.get_property(stream, "__controller__");
                let _ = vm.call(abort_fn, sink, &[reason, controller]);
            }
        }

        let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
        vm.resolve_promise(p, reason);
        Ok(p)
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
            let pending_writes = vm.alloc_array(Vec::new());
            vm.set_property(instance, "__pending_writes__", pending_writes);

            let state = vm.get_property(stream, "__state__");
            let state_str = vm.to_string_value(state);

            let ready = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            if state_str == "errored" {
                let err = vm.get_property(stream, "__stored_error__");
                vm.reject_promise(ready, err);
            } else {
                vm.resolve_promise(ready, JsValue::undefined());
            }
            vm.set_property(instance, "__ready_promise__", ready);

            let closed = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());
            if state_str == "closed" {
                vm.resolve_promise(closed, JsValue::undefined());
            } else if state_str == "errored" {
                let err = vm.get_property(stream, "__stored_error__");
                vm.reject_promise(closed, err);
            }
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
        let state = vm.get_property(this, "__state__");
        let state_str = vm.to_string_value(state);
        let p = vm.alloc_promise(PromiseState::Pending, JsValue::undefined());

        if state_str == "errored" {
            let err = vm.get_property(this, "__stored_error__");
            vm.reject_promise(p, err);
            return Ok(p);
        }
        if state_str == "closed" {
            let err = vm.alloc_string("Cannot close an already-closed WritableStream".into());
            vm.reject_promise(p, err);
            return Ok(p);
        }

        let closed_str = vm.alloc_string("closed".into());
        vm.set_property(this, "__state__", closed_str);

        let sink = vm.get_property(this, "__sink__");
        if sink.is_js_object() {
            let close_fn = vm.get_property(sink, "close");
            if vm.is_callable(close_fn) {
                let controller = vm.get_property(this, "__controller__");
                match vm.call(close_fn, sink, &[controller]) {
                    Ok(res) => {
                        if res.is_js_object() {
                            let then_fn = vm.get_property(res, "then");
                            if vm.is_callable(then_fn) {
                                let p_clone = p;
                                let stream_val = this;
                                let on_fulfilled = vm.alloc_closure_native_function(
                                    "onCloseFulfilled",
                                    0,
                                    Arc::new(move |vm, _t, _a| {
                                        let writer = vm.get_property(stream_val, "__writer__");
                                        if writer.is_js_object() {
                                            let closed_prom =
                                                vm.get_property(writer, "__closed_promise__");
                                            vm.resolve_promise(closed_prom, JsValue::undefined());
                                        }
                                        vm.resolve_promise(p_clone, JsValue::undefined());
                                        Ok(JsValue::undefined())
                                    }),
                                );
                                let on_rejected = vm.alloc_closure_native_function(
                                    "onCloseRejected",
                                    1,
                                    Arc::new(move |vm, _t, r_args| {
                                        let err =
                                            r_args.first().copied().unwrap_or(JsValue::undefined());
                                        let writer = vm.get_property(stream_val, "__writer__");
                                        error_writable_stream_and_writer(
                                            vm, stream_val, writer, err,
                                        );
                                        vm.reject_promise(p_clone, err);
                                        Ok(JsValue::undefined())
                                    }),
                                );
                                let _ = vm.call(then_fn, res, &[on_fulfilled, on_rejected]);
                                return Ok(p);
                            }
                        }
                    }
                    Err(err) => {
                        let err_val = match err {
                            VmError::Thrown(v) => v,
                            e => vm.alloc_string(e.to_string()),
                        };
                        let writer = vm.get_property(this, "__writer__");
                        error_writable_stream_and_writer(vm, this, writer, err_val);
                        vm.reject_promise(p, err_val);
                        return Ok(p);
                    }
                }
            }
        }

        let writer = vm.get_property(this, "__writer__");
        if writer.is_js_object() {
            let closed_prom = vm.get_property(writer, "__closed_promise__");
            vm.resolve_promise(closed_prom, JsValue::undefined());
        }

        vm.resolve_promise(p, JsValue::undefined());
        Ok(p)
    });
    vm.set_property(stream_proto, "close", stream_close);

    let stream_abort = vm.alloc_native_function("abort", 1, |vm, this, args| {
        let reason = args.first().copied().unwrap_or(JsValue::undefined());
        let writer = vm.get_property(this, "__writer__");
        error_writable_stream_and_writer(vm, this, writer, reason);

        let sink = vm.get_property(this, "__sink__");
        if sink.is_js_object() {
            let abort_fn = vm.get_property(sink, "abort");
            if vm.is_callable(abort_fn) {
                let controller = vm.get_property(this, "__controller__");
                let _ = vm.call(abort_fn, sink, &[reason, controller]);
            }
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
            let writer = vm.get_property(stream, "__writer__");
            error_writable_stream_and_writer(vm, stream, writer, err);
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
