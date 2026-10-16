//! The `console` global object.

use super::*;

pub(super) fn register_console(vm: &mut Vm) {
    let log_fn = vm.alloc_native_function("log", 1, native_console_log);
    let error_fn = vm.alloc_native_function("error", 1, native_console_error);
    let warn_fn = vm.alloc_native_function("warn", 1, native_console_warn);
    let info_fn = vm.alloc_native_function("info", 1, native_console_info);

    let console_obj = vm.alloc_object();
    vm.set_property(console_obj, "log", log_fn);
    vm.set_property(console_obj, "error", error_fn);
    vm.set_property(console_obj, "warn", warn_fn);
    vm.set_property(console_obj, "info", info_fn);

    vm.globals.insert("console".to_string(), console_obj);
}

pub(super) fn native_console_log(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    println!("{}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_error(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    eprintln!("{}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_warn(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    eprintln!("[WARN] {}", parts.join(" "));
    Ok(JsValue::undefined())
}

fn native_console_info(vm: &mut Vm, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let parts: Vec<String> = args.iter().map(|arg| vm.to_string_value(*arg)).collect();
    println!("[INFO] {}", parts.join(" "));
    Ok(JsValue::undefined())
}
