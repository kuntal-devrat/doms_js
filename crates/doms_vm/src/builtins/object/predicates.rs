//! `Object.prototype.hasOwnProperty` and `propertyIsEnumerable`.

use super::descriptor::get_own_property_descriptor_internal;
use super::*;

pub(super) fn register_predicates(vm: &mut Vm, object_proto: JsValue) {
    let has_own_prop = vm.alloc_native_function("hasOwnProperty", 1, |vm, this, args| {
        if this.is_nullish() {
            return Err(VmError::TypeError(
                "Object.prototype.hasOwnProperty called on null or undefined".into(),
            ));
        }
        let this_obj = if this.is_js_object() { this } else { vm.to_object_value(this)? };
        let prop = args.first().copied().unwrap_or(JsValue::undefined());
        let key = vm.to_property_key(prop)?;
        if vm.module_namespace_objects.contains(&this_obj) {
            if let PropertyKeyVal::String(s) = &key {
                if vm.has_own_property_key(this_obj, &key)
                    && vm.get_property(this_obj, s) == JsValue::uninitialized()
                {
                    return Err(VmError::ReferenceError(format!(
                        "Cannot access uninitialized export '{}'",
                        s
                    )));
                }
            }
        }
        let desc = get_own_property_descriptor_internal(vm, this_obj, &key)?;
        Ok(JsValue::from_boolean(!desc.is_undefined()))
    });
    vm.set_property(object_proto, "hasOwnProperty", has_own_prop);

    let prop_is_enumerable =
        vm.alloc_native_function("propertyIsEnumerable", 1, |vm, this, args| {
            if this.is_nullish() {
                return Err(VmError::TypeError(
                    "Object.prototype.propertyIsEnumerable called on null or undefined".into(),
                ));
            }
            let this_obj = if this.is_js_object() { this } else { vm.to_object_value(this)? };
            let prop = args.first().copied().unwrap_or(JsValue::undefined());
            let key = vm.to_property_key(prop)?;
            if vm.module_namespace_objects.contains(&this_obj) {
                if let PropertyKeyVal::String(s) = &key {
                    if vm.has_own_property_key(this_obj, &key)
                        && vm.get_property(this_obj, s) == JsValue::uninitialized()
                    {
                        return Err(VmError::ReferenceError(format!(
                            "Cannot access uninitialized export '{}'",
                            s
                        )));
                    }
                }
            }
            let desc = get_own_property_descriptor_internal(vm, this_obj, &key)?;
            if desc.is_undefined() {
                return Ok(JsValue::from_boolean(false));
            }
            let is_enum = vm.get_property(desc, "enumerable").is_truthy();
            Ok(JsValue::from_boolean(is_enum))
        });
    vm.set_property(object_proto, "propertyIsEnumerable", prop_is_enumerable);
}
