//! `Event`, `CustomEvent` and `EventTarget`.

use super::*;

pub(super) fn register_events(vm: &mut Vm) {
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
        let detail =
            if options.is_object() { vm.get_property(options, "detail") } else { JsValue::null() };
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
