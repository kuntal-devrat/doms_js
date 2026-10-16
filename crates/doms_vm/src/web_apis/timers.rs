//! `setTimeout`, `setInterval` and `queueMicrotask`.

use super::*;

pub(super) fn register_timers(vm: &mut Vm) {
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
