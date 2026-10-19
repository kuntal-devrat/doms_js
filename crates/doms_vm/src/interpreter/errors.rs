//! Conversion between interpreter errors and JavaScript error objects.

use super::*;

impl Vm {
    pub fn alloc_error_opt(&mut self, name: String, message: Option<String>) -> JsValue {
        let ctor = self.globals.get(&name).copied().unwrap_or(JsValue::undefined());
        let proto = if ctor.is_object() {
            let p = self.get_property(ctor, "prototype");
            if p.is_object() { Some(p) } else { None }
        } else {
            None
        };
        let msg_str = message.clone().unwrap_or_default();
        let obj = JsObject::new_error(ShapeRegistry::ROOT_SHAPE_ID, proto, name.clone(), msg_str);
        let ptr = self.allocate_js_object(obj);
        let val = unsafe { JsValue::from_object_ptr(ptr as *mut u8) };
        if let Some(msg) = message {
            let prev_temp = self.temp_roots.len();
            self.temp_roots.push(val);
            let msg_val = self.alloc_string(msg);
            let cur_val = self.temp_roots[prev_temp];
            self.define_builtin_property(cur_val, "message", msg_val, crate::shape::PropertyAttributes::BUILTIN);
            self.temp_roots.pop();
            self.temp_roots.truncate(prev_temp);
            cur_val
        } else {
            val
        }
    }

    pub fn alloc_error(&mut self, name: String, message: String) -> JsValue {
        self.alloc_error_opt(name, Some(message))
    }

    pub fn vm_error_to_js_value(&mut self, err: &VmError) -> JsValue {
        match err {
            VmError::Thrown(val) => *val,
            VmError::TypeError(msg) => self.alloc_error("TypeError".into(), msg.clone()),
            VmError::ReferenceError(msg) => self.alloc_error("ReferenceError".into(), msg.clone()),
            VmError::RangeError(msg) => self.alloc_error("RangeError".into(), msg.clone()),
            VmError::SyntaxError(msg) => self.alloc_error("SyntaxError".into(), msg.clone()),
            VmError::UriError(msg) => self.alloc_error("URIError".into(), msg.clone()),
            VmError::RuntimeError(msg) | VmError::UndefinedGlobal(msg) => {
                self.alloc_error("Error".into(), msg.clone())
            }
            VmError::StackUnderflow | VmError::InvalidInstruction(_) => {
                self.alloc_error("Error".into(), err.to_string())
            }
        }
    }

    pub fn handle_exception_at_depth(&mut self, err: VmError, min_frame_idx: usize) -> Result<(), VmError> {
        if let VmError::RuntimeError(ref msg) = err {
            if msg.contains("Execution step limit exceeded") || msg.contains("Execution timeout exceeded") {
                return Err(err);
            }
        }
        if let VmError::RangeError(ref msg) = err {
            if msg.contains("Out of memory") {
                return Err(err);
            }
        }
        if let Some(handler) = self.catch_stack.last() {
            if handler.frame_idx >= min_frame_idx {
                let handler = self.catch_stack.pop().unwrap();
                let err_val = self.vm_error_to_js_value(&err);
                self.frames.truncate(handler.frame_idx + 1);
                self.stack.truncate(handler.stack_len);
                self.push(err_val);
                if let Some(frame) = self.frames.last_mut() {
                    frame.scopes.truncate(handler.scopes_len);
                    frame.ip = handler.target_ip;
                }
                return Ok(());
            }
        }
        Err(err)
    }

    pub fn handle_exception(&mut self, err: VmError) -> Result<(), VmError> {
        self.handle_exception_at_depth(err, 0)
    }
}
