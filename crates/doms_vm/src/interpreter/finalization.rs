//! `FinalizationRegistry` method dispatch.

use super::*;

impl Vm {
    pub(crate) fn get_finalization_registry_method(&mut self, target: JsValue, name: &str) -> Option<JsValue> {
        match name {
            "register" => Some(self.alloc_closure_native_function("register", 3, Arc::new(move |_vm, _this, args| {
                let target_obj = args.first().copied().unwrap_or(JsValue::undefined());
                let held_value = args.get(1).copied().unwrap_or(JsValue::undefined());
                let unreg_token = args.get(2).copied();
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::FinalizationRegistry { entries, .. } = &mut obj.kind {
                        entries.push((target_obj, held_value, unreg_token));
                    }
                }
                Ok(JsValue::undefined())
            }))),
            "unregister" => Some(self.alloc_closure_native_function("unregister", 1, Arc::new(move |_vm, _this, args| {
                let token = args.first().copied().unwrap_or(JsValue::undefined());
                let mut removed = false;
                if let Some(ptr) = target.as_object_ptr() {
                    let obj = unsafe { &mut *(ptr as *mut JsObject) };
                    if let ObjectKind::FinalizationRegistry { entries, .. } = &mut obj.kind {
                        let len_before = entries.len();
                        entries.retain(|(_, _, tok)| {
                            if let Some(t) = tok {
                                !crate::object::js_values_equal(*t, token)
                            } else {
                                true
                            }
                        });
                        removed = entries.len() < len_before;
                    }
                }
                Ok(JsValue::from_boolean(removed))
            }))),
            _ => None,
        }
    }
}
