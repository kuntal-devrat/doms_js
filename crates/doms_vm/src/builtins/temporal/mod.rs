//! Minimal `Temporal` namespace shims.

use super::*;

pub(super) fn register_temporal_builtins(vm: &mut Vm) {
    // Temporal namespace object
    let temporal_obj = vm.alloc_object();

    let instant_ctor = vm.alloc_native_function("Instant", 1, |vm, _this, args| {
        let ms = args.first().and_then(|v| v.to_number()).unwrap_or(0.0);
        let inst = vm.alloc_object();
        vm.set_property(inst, "epochMilliseconds", JsValue::from_double(ms));
        vm.set_property(inst, "epochNanoseconds", JsValue::from_double(ms * 1_000_000.0));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let ms_val = vm.get_property(this, "epochMilliseconds")
                .to_number()
                .unwrap_or(0.0);
            Ok(vm.alloc_string(format!("{}Z", ms_val)))
        });
        vm.set_property(inst, "toString", to_str);
        Ok(inst)
    });
    vm.set_property(temporal_obj, "Instant", instant_ctor);

    let now_obj = vm.alloc_object();
    let instant_fn = vm.alloc_native_function("instant", 0, |vm, _this, _args| {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        let inst = vm.alloc_object();
        vm.set_property(inst, "epochMilliseconds", JsValue::from_double(ms));
        vm.set_property(inst, "epochNanoseconds", JsValue::from_double(ms * 1_000_000.0));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let ms_val = vm.get_property(this, "epochMilliseconds")
                .to_number()
                .unwrap_or(0.0);
            Ok(vm.alloc_string(format!("{}Z", ms_val)))
        });
        vm.set_property(inst, "toString", to_str);
        Ok(inst)
    });
    vm.set_property(now_obj, "instant", instant_fn);
    vm.set_property(temporal_obj, "Now", now_obj);

    let plain_date_ctor = vm.alloc_native_function("PlainDate", 3, |vm, _this, args| {
        let y = args.first().and_then(|v| v.to_int32()).unwrap_or(1970);
        let m = args.get(1).and_then(|v| v.to_int32()).unwrap_or(1);
        let d = args.get(2).and_then(|v| v.to_int32()).unwrap_or(1);
        let date_obj = vm.alloc_object();
        vm.set_property(date_obj, "year", JsValue::from_int32(y));
        vm.set_property(date_obj, "month", JsValue::from_int32(m));
        vm.set_property(date_obj, "day", JsValue::from_int32(d));
        let to_str = vm.alloc_native_function("toString", 0, |vm, this, _args| {
            let y_val = vm.get_property(this, "year").to_int32().unwrap_or(0);
            let m_val = vm.get_property(this, "month").to_int32().unwrap_or(0);
            let d_val = vm.get_property(this, "day").to_int32().unwrap_or(0);
            Ok(vm.alloc_string(format!("{:04}-{:02}-{:02}", y_val, m_val, d_val)))
        });
        vm.set_property(date_obj, "toString", to_str);
        Ok(date_obj)
    });
    vm.set_property(temporal_obj, "PlainDate", plain_date_ctor);

    vm.globals.insert("Temporal".to_string(), temporal_obj);
}
