//! Complete ECMAScript Meta-Object Protocol (MOP) implementation for Proxy and Reflect.
//!
//! Implements all 13 internal methods per ECMAScript 2024 Chapter 10.5:
//! 1.  [[GetPrototypeOf]]
//! 2.  [[SetPrototypeOf]]
//! 3.  [[IsExtensible]]
//! 4.  [[PreventExtensions]]
//! 5.  [[GetOwnProperty]]
//! 6.  [[DefineOwnProperty]]
//! 7.  [[HasProperty]]
//! 8.  [[Get]]
//! 9.  [[Set]]
//! 10. [[Delete]]
//! 11. [[OwnPropertyKeys]]
//! 12. [[Call]]
//! 13. [[Construct]]

use super::*;
use crate::builtins::object::descriptor::get_own_property_descriptor_internal;
use crate::builtins::object::integrity::is_extensible_internal;
use crate::builtins::object::names::get_own_property_keys;
use crate::interpreter::PropertyKeyVal;

impl Vm {
    /// Validates that `proxy` is a Proxy object and returns `(target, handler, Option<trap>)`.
    /// Throws TypeError if the proxy has been revoked or if the trap is not callable.
    pub fn validate_proxy_and_get_trap(
        &mut self,
        proxy: JsValue,
        trap_name: &str,
    ) -> Result<(JsValue, JsValue, Option<JsValue>), VmError> {
        let ptr = proxy.as_object_ptr().ok_or_else(|| {
            VmError::TypeError("Proxy operation requires an object".into())
        })?;
        let (target, handler) = {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Proxy { target, handler } => (*target, *handler),
                _ => return Err(VmError::TypeError("Object is not a Proxy".into())),
            }
        };

        if handler.is_null() || handler.is_undefined() {
            return Err(VmError::TypeError("Cannot perform operation on a proxy that has been revoked".into()));
        }

        let trap = self.get_property_result(handler, trap_name)?;
        if trap.is_undefined() || trap.is_null() {
            return Ok((target, handler, None));
        }

        if !self.is_callable(trap) {
            return Err(VmError::TypeError(format!("Proxy trap '{}' is not callable", trap_name)));
        }

        Ok((target, handler, Some(trap)))
    }

    /// 10.5.1 [[GetPrototypeOf]]
    pub fn proxy_get_prototype_of(&mut self, proxy: JsValue) -> Result<JsValue, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "getPrototypeOf")?;
        if let Some(trap) = trap_opt {
            let handler_proto = self.call(trap, handler, &[target])?;
            if !handler_proto.is_js_object() && !handler_proto.is_null() {
                return Err(VmError::TypeError("Proxy getPrototypeOf handler returned non-object or non-null".into()));
            }
            let is_ext = is_extensible_internal(self, target)?;
            if !is_ext {
                let target_proto = self.get_prototype_of_val(target)?;
                if handler_proto != target_proto {
                    return Err(VmError::TypeError("Proxy getPrototypeOf invariant: target not extensible must return same prototype".into()));
                }
            }
            Ok(handler_proto)
        } else {
            self.get_prototype_of_val(target)
        }
    }

    /// 10.5.2 [[SetPrototypeOf]]
    pub fn proxy_set_prototype_of(&mut self, proxy: JsValue, proto: JsValue) -> Result<bool, VmError> {
        if !proto.is_js_object() && !proto.is_null() {
            return Err(VmError::TypeError("Object prototype may only be an Object or null".into()));
        }
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "setPrototypeOf")?;
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target, proto])?;
            let boolean_trap_result = trap_res.is_truthy();
            if !boolean_trap_result {
                return Ok(false);
            }
            let is_ext = is_extensible_internal(self, target)?;
            if !is_ext {
                let target_proto = self.get_prototype_of_val(target)?;
                if proto != target_proto {
                    return Err(VmError::TypeError("Proxy setPrototypeOf invariant: cannot change prototype of non-extensible target".into()));
                }
            }
            Ok(true)
        } else {
            self.set_prototype_of_val(target, proto)
        }
    }

    /// 10.5.3 [[IsExtensible]]
    pub fn proxy_is_extensible(&mut self, proxy: JsValue) -> Result<bool, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "isExtensible")?;
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target])?;
            let boolean_trap_result = trap_res.is_truthy();
            let target_result = is_extensible_internal(self, target)?;
            if boolean_trap_result != target_result {
                return Err(VmError::TypeError("Proxy isExtensible invariant: result must match target extensibility".into()));
            }
            Ok(boolean_trap_result)
        } else {
            is_extensible_internal(self, target)
        }
    }

    /// 10.5.4 [[PreventExtensions]]
    pub fn proxy_prevent_extensions(&mut self, proxy: JsValue) -> Result<bool, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "preventExtensions")?;
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target])?;
            let boolean_trap_result = trap_res.is_truthy();
            if boolean_trap_result {
                let target_result = is_extensible_internal(self, target)?;
                if target_result {
                    return Err(VmError::TypeError("Proxy preventExtensions invariant: cannot return true if target is extensible".into()));
                }
            }
            Ok(boolean_trap_result)
        } else {
            crate::builtins::object::integrity::prevent_extensions_internal(self, target)
        }
    }

    /// 10.5.5 [[GetOwnProperty]]
    pub fn proxy_get_own_property_descriptor(&mut self, proxy: JsValue, key: &PropertyKeyVal) -> Result<JsValue, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "getOwnPropertyDescriptor")?;
        let key_arg = self.property_key_to_value(key);
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target, key_arg])?;
            if !trap_res.is_js_object() && !trap_res.is_undefined() {
                return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor trap must return an object or undefined".into()));
            }
            let target_desc = get_own_property_descriptor_internal(self, target, key)?;
            if trap_res.is_undefined() {
                if !target_desc.is_undefined() {
                    let conf = self.get_property(target_desc, "configurable");
                    if !conf.is_truthy() {
                        return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor invariant: cannot report non-configurable property as undefined".into()));
                    }
                    let is_ext = is_extensible_internal(self, target)?;
                    if !is_ext {
                        return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor invariant: cannot report property on non-extensible target as undefined".into()));
                    }
                }
                return Ok(JsValue::undefined());
            }

            // Invariants per ES 10.5.5 steps 22 and 23
            let trap_conf = self.get_property(trap_res, "configurable").is_truthy();
            if !trap_conf {
                if target_desc.is_undefined() {
                    return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor invariant: cannot report non-configurable property for non-existent target property".into()));
                }
                let target_conf = self.get_property(target_desc, "configurable").is_truthy();
                if target_conf {
                    return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor invariant: cannot report non-configurable property for configurable target property".into()));
                }
                let trap_writ = self.get_property(trap_res, "writable");
                if !trap_writ.is_undefined() && !trap_writ.is_truthy() {
                    let target_writ = self.get_property(target_desc, "writable");
                    if !target_writ.is_undefined() && target_writ.is_truthy() {
                        return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor invariant: cannot report non-writable for writable target property".into()));
                    }
                }
            } else if !target_desc.is_undefined() {
                let target_conf = self.get_property(target_desc, "configurable").is_truthy();
                if !target_conf {
                    return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor invariant: cannot report non-configurable property as configurable".into()));
                }
            } else {
                let is_ext = is_extensible_internal(self, target)?;
                if !is_ext {
                    return Err(VmError::TypeError("Proxy getOwnPropertyDescriptor invariant: cannot report non-existent property on non-extensible target".into()));
                }
            }
            Ok(trap_res)
        } else {
            get_own_property_descriptor_internal(self, target, key)
        }
    }

    /// 10.5.6 [[DefineOwnProperty]]
    pub fn proxy_define_own_property(&mut self, proxy: JsValue, key: &PropertyKeyVal, desc: JsValue) -> Result<bool, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "defineProperty")?;
        let key_arg = self.property_key_to_value(key);
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target, key_arg, desc])?;
            let boolean_trap_result = trap_res.is_truthy();
            if !boolean_trap_result {
                return Ok(false);
            }
            let target_desc = get_own_property_descriptor_internal(self, target, key)?;
            let is_ext = is_extensible_internal(self, target)?;
            let desc_conf = self.get_property(desc, "configurable");
            let setting_config_false = !desc_conf.is_undefined() && !desc_conf.is_truthy();
            if target_desc.is_undefined() {
                if !is_ext {
                    return Err(VmError::TypeError("Proxy defineProperty invariant: cannot define property on non-extensible target".into()));
                }
                if setting_config_false {
                    return Err(VmError::TypeError("Proxy defineProperty invariant: cannot define non-configurable property that does not exist on target".into()));
                }
            } else {
                let target_conf = self.get_property(target_desc, "configurable").is_truthy();
                if setting_config_false && target_conf {
                    return Err(VmError::TypeError("Proxy defineProperty invariant: cannot define non-configurable property for configurable target property".into()));
                }
                if !target_conf {
                    if !desc_conf.is_undefined() && desc_conf.is_truthy() {
                        return Err(VmError::TypeError("Proxy defineProperty invariant: cannot make non-configurable property configurable".into()));
                    }
                    let target_writ = self.get_property(target_desc, "writable");
                    let desc_writ = self.get_property(desc, "writable");
                    if !target_writ.is_undefined() && target_writ.is_truthy() {
                        if !desc_writ.is_undefined() && !desc_writ.is_truthy() {
                            return Err(VmError::TypeError("Proxy defineProperty invariant: cannot change writable property to non-writable".into()));
                        }
                    }
                    if !target_writ.is_undefined() && !target_writ.is_truthy() {
                        if !desc_writ.is_undefined() && desc_writ.is_truthy() {
                            return Err(VmError::TypeError("Proxy defineProperty invariant: cannot make non-writable property writable".into()));
                        }
                        let desc_val = self.get_property(desc, "value");
                        let target_val = self.get_property(target_desc, "value");
                        if !desc_val.is_undefined() && !self.same_value(desc_val, target_val) {
                            return Err(VmError::TypeError("Proxy defineProperty invariant: cannot change value of non-configurable non-writable property".into()));
                        }
                    }
                }
            }
            Ok(true)
        } else {
            let res = crate::builtins::object::define_property::define_property_internal(self, target, key.clone(), desc);
            Ok(res.is_ok())
        }
    }

    /// 10.5.7 [[HasProperty]]
    pub fn proxy_has(&mut self, proxy: JsValue, key: &PropertyKeyVal) -> Result<bool, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "has")?;
        let key_arg = self.property_key_to_value(key);
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target, key_arg])?;
            let boolean_trap_result = trap_res.is_truthy();
            if !boolean_trap_result {
                let target_desc = get_own_property_descriptor_internal(self, target, key)?;
                if !target_desc.is_undefined() {
                    let conf = self.get_property(target_desc, "configurable").is_truthy();
                    if !conf {
                        return Err(VmError::TypeError("Proxy has invariant: cannot report false for non-configurable property".into()));
                    }
                    let is_ext = is_extensible_internal(self, target)?;
                    if !is_ext {
                        return Err(VmError::TypeError("Proxy has invariant: cannot report false for existing property on non-extensible target".into()));
                    }
                }
            }
            Ok(boolean_trap_result)
        } else {
            self.has_property_key(target, key)
        }
    }

    /// 10.5.8 [[Get]]
    pub fn proxy_get(&mut self, proxy: JsValue, key: &PropertyKeyVal, receiver: JsValue) -> Result<JsValue, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "get")?;
        let key_arg = self.property_key_to_value(key);
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target, key_arg, receiver])?;
            let target_desc = get_own_property_descriptor_internal(self, target, key)?;
            if !target_desc.is_undefined() {
                let conf = self.get_property(target_desc, "configurable").is_truthy();
                let has_val = !self.get_property(target_desc, "value").is_undefined() || self.has_property(target_desc, "value").unwrap_or(false);
                let is_writ = self.get_property(target_desc, "writable").is_truthy();
                if !conf && has_val && !is_writ {
                    let target_val = self.get_property(target_desc, "value");
                    if !self.same_value(trap_res, target_val) {
                        return Err(VmError::TypeError("Proxy get invariant: cannot report different value for non-writable non-configurable property".into()));
                    }
                }
                let get_fn = self.get_property(target_desc, "get");
                if !conf && !has_val && get_fn.is_undefined() && !trap_res.is_undefined() {
                    return Err(VmError::TypeError("Proxy get invariant: cannot report value for non-configurable accessor with undefined getter".into()));
                }
            }
            Ok(trap_res)
        } else {
            match key {
                PropertyKeyVal::String(s) => Ok(self.get_property_with_receiver(target, s, receiver)),
                PropertyKeyVal::Symbol(sym) => Ok(self.get_symbol_property(target, *sym)),
            }
        }
    }

    /// 10.5.9 [[Set]]
    pub fn proxy_set(&mut self, proxy: JsValue, key: &PropertyKeyVal, value: JsValue, receiver: JsValue) -> Result<bool, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "set")?;
        let key_arg = self.property_key_to_value(key);
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target, key_arg, value, receiver])?;
            let boolean_trap_result = trap_res.is_truthy();
            if !boolean_trap_result {
                return Ok(false);
            }
            let target_desc = get_own_property_descriptor_internal(self, target, key)?;
            if !target_desc.is_undefined() {
                let conf = self.get_property(target_desc, "configurable").is_truthy();
                let has_val = !self.get_property(target_desc, "value").is_undefined() || self.has_property(target_desc, "value").unwrap_or(false);
                let is_writ = self.get_property(target_desc, "writable").is_truthy();
                if !conf && has_val && !is_writ {
                    let target_val = self.get_property(target_desc, "value");
                    if !self.same_value(value, target_val) {
                        return Err(VmError::TypeError("Proxy set invariant: cannot set different value for non-writable non-configurable property".into()));
                    }
                }
                let set_fn = self.get_property(target_desc, "set");
                if !conf && !has_val && set_fn.is_undefined() {
                    return Err(VmError::TypeError("Proxy set invariant: cannot set value for non-configurable accessor with undefined setter".into()));
                }
            }
            Ok(true)
        } else {
            match key {
                PropertyKeyVal::String(s) => self.set_property_with_receiver(target, s, value, receiver),
                PropertyKeyVal::Symbol(sym) => self.set_symbol_property_result(target, *sym, value).map(|_| true),
            }
        }
    }

    /// 10.5.10 [[Delete]]
    pub fn proxy_delete_property(&mut self, proxy: JsValue, key: &PropertyKeyVal) -> Result<bool, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "deleteProperty")?;
        let key_arg = self.property_key_to_value(key);
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target, key_arg])?;
            let boolean_trap_result = trap_res.is_truthy();
            if !boolean_trap_result {
                return Ok(false);
            }
            let target_desc = get_own_property_descriptor_internal(self, target, key)?;
            if !target_desc.is_undefined() {
                let conf = self.get_property(target_desc, "configurable").is_truthy();
                if !conf {
                    return Err(VmError::TypeError("Proxy deleteProperty invariant: cannot delete non-configurable property".into()));
                }
                let is_ext = is_extensible_internal(self, target)?;
                if !is_ext {
                    return Err(VmError::TypeError("Proxy deleteProperty invariant: cannot delete property on non-extensible target".into()));
                }
            }
            Ok(true)
        } else {
            self.delete_property_key(target, key)
        }
    }

    /// 10.5.11 [[OwnPropertyKeys]]
    pub fn proxy_own_property_keys(&mut self, proxy: JsValue) -> Result<Vec<PropertyKeyVal>, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "ownKeys")?;
        if let Some(trap) = trap_opt {
            let trap_res = self.call(trap, handler, &[target])?;
            if !trap_res.is_js_object() {
                return Err(VmError::TypeError("Proxy ownKeys trap must return an object".into()));
            }
            let keys_vec = self.create_list_from_array_like(trap_res)?;
            let mut result_keys = Vec::with_capacity(keys_vec.len());
            for k_val in keys_vec {
                if !k_val.is_js_string() && !k_val.is_js_symbol() {
                    return Err(VmError::TypeError("Proxy ownKeys trap elements must be String or Symbol".into()));
                }
                let key = self.to_property_key(k_val)?;
                if result_keys.contains(&key) {
                    return Err(VmError::TypeError("Proxy ownKeys trap cannot list duplicate property keys".into()));
                }
                result_keys.push(key);
            }

            let target_keys = get_own_property_keys(self, target)?;
            let is_ext = is_extensible_internal(self, target)?;

            for tk in &target_keys {
                let desc = get_own_property_descriptor_internal(self, target, tk)?;
                let is_conf = if !desc.is_undefined() {
                    self.get_property(desc, "configurable").is_truthy()
                } else {
                    true
                };
                if !is_conf && !result_keys.contains(tk) {
                    return Err(VmError::TypeError("Proxy ownKeys invariant: must include all non-configurable properties".into()));
                }
                if !is_ext && !result_keys.contains(tk) {
                    return Err(VmError::TypeError("Proxy ownKeys invariant: non-extensible target must include all properties".into()));
                }
            }

            if !is_ext {
                for rk in &result_keys {
                    if !target_keys.contains(rk) {
                        return Err(VmError::TypeError("Proxy ownKeys invariant: non-extensible target cannot report extra properties".into()));
                    }
                }
            }

            Ok(result_keys)
        } else {
            get_own_property_keys(self, target)
        }
    }

    /// 10.5.12 [[Call]]
    pub fn proxy_call(&mut self, proxy: JsValue, this_arg: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "apply")?;
        if !self.is_callable(target) {
            return Err(VmError::TypeError("Proxy target is not callable".into()));
        }
        if let Some(trap) = trap_opt {
            let arg_arr = self.alloc_array(args.to_vec());
            self.call(trap, handler, &[target, this_arg, arg_arr])
        } else {
            self.call(target, this_arg, args)
        }
    }

    /// 10.5.13 [[Construct]]
    pub fn proxy_construct(&mut self, proxy: JsValue, args: &[JsValue], new_target: JsValue) -> Result<JsValue, VmError> {
        let (target, handler, trap_opt) = self.validate_proxy_and_get_trap(proxy, "construct")?;
        if !self.is_constructor(target) {
            return Err(VmError::TypeError("Proxy target is not a constructor".into()));
        }
        if let Some(trap) = trap_opt {
            let arg_arr = self.alloc_array(args.to_vec());
            let new_target_val = if new_target.is_undefined() { proxy } else { new_target };
            let res = self.call(trap, handler, &[target, arg_arr, new_target_val])?;
            if !res.is_js_object() {
                return Err(VmError::TypeError("Proxy construct trap must return an object".into()));
            }
            Ok(res)
        } else {
            self.construct_with_new_target(target, args, new_target)
        }
    }

    // Helper functions
    pub(crate) fn property_key_to_value(&mut self, key: &PropertyKeyVal) -> JsValue {
        match key {
            PropertyKeyVal::String(s) => self.alloc_string(s.clone()),
            PropertyKeyVal::Symbol(sym) => *sym,
        }
    }

    pub(crate) fn get_prototype_of_val(&mut self, target: JsValue) -> Result<JsValue, VmError> {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                return self.proxy_get_prototype_of(target);
            }
            Ok(obj.prototype.unwrap_or(JsValue::null()))
        } else if target.is_string() {
            Ok(self.string_proto.unwrap_or(JsValue::null()))
        } else if target.is_number() {
            Ok(self.number_proto.unwrap_or(JsValue::null()))
        } else if target.is_boolean() {
            Ok(self.boolean_proto.unwrap_or(JsValue::null()))
        } else if target.is_symbol() {
            Ok(self.symbol_proto.unwrap_or(JsValue::null()))
        } else {
            Ok(JsValue::null())
        }
    }

    pub(crate) fn set_prototype_of_val(&mut self, target: JsValue, proto: JsValue) -> Result<bool, VmError> {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                return self.proxy_set_prototype_of(target, proto);
            }
            let current = obj.prototype.unwrap_or(JsValue::null());
            if current == proto {
                return Ok(true);
            }
            if self.non_extensible_objects.contains(&target) {
                return Ok(false);
            }
            // Check cycle
            let mut p = proto;
            while let Some(p_ptr) = p.as_object_ptr() {
                if p == target {
                    return Ok(false);
                }
                let p_obj = unsafe { &*(p_ptr as *mut JsObject) };
                if let Some(next_p) = p_obj.prototype {
                    p = next_p;
                } else {
                    break;
                }
            }
            obj.prototype = if proto.is_null() { None } else { Some(proto) };
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) fn is_extensible_val(&mut self, target: JsValue) -> Result<bool, VmError> {
        if !target.is_object() {
            return Ok(false);
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                return self.proxy_is_extensible(target);
            }
        }
        Ok(!self.non_extensible_objects.contains(&target))
    }

    pub(crate) fn prevent_extensions_val(&mut self, target: JsValue) -> Result<bool, VmError> {
        if !target.is_object() {
            return Ok(false);
        }
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::Proxy { .. }) {
                return self.proxy_prevent_extensions(target);
            }
        }
        if self.module_namespace_objects.contains(&target) {
            return Ok(true);
        }
        self.non_extensible_objects.insert(target);
        Ok(true)
    }

    pub(crate) fn create_list_from_array_like(&mut self, array_like: JsValue) -> Result<Vec<JsValue>, VmError> {
        if !array_like.is_object() {
            return Err(VmError::TypeError("CreateListFromArrayLike called on non-object".into()));
        }
        if let Some(ptr) = array_like.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &obj.kind {
                if obj.accessors.is_none() {
                    return Ok(elements.iter().map(|v| if v.is_hole() { JsValue::undefined() } else { *v }).collect());
                }
            }
        }
        let len_val = self.get_property_result(array_like, "length")?;
        let len = len_val
            .to_int32()
            .or_else(|| len_val.to_number().map(|n| n as i32))
            .unwrap_or(0)
            .max(0) as usize;
        let mut list = Vec::with_capacity(len.min(100_000));
        for i in 0..len {
            let elem = self.get_property_result(array_like, &i.to_string())?;
            list.push(elem);
        }
        Ok(list)
    }
}
