use super::*;

impl Vm {
    pub fn capture_upvalues(&mut self) -> Vec<JsValue> {
        let mut upvalues = Vec::new();
        if let Some(current_frame) = self.frames.last() {
            if current_frame.in_param_init {
                return current_frame.scopes.clone();
            }
            upvalues.extend_from_slice(&current_frame.scopes);
            if !current_frame.local_names.is_empty() || current_frame.arguments.is_some() {
                if let Some(env) = current_frame.env_obj {
                    upvalues.push(env);
                    return upvalues;
                }
            } else {
                return upvalues;
            }
        } else {
            return upvalues;
        }

        let (local_names, locals, arguments, callee_name, callee_val) = {
            let frame = self.frames.last().unwrap();
            let c_name = frame.callee_func.as_ref().and_then(|f| f.name.clone());
            (frame.local_names.clone(), frame.locals.clone(), frame.arguments, c_name, frame.callee_val)
        };
        let env = self.alloc_object();
        for (i, name) in local_names.iter().enumerate() {
            let val = locals.get(i).copied().unwrap_or(JsValue::undefined());
            self.set_property(env, name, val);
        }
        if let Some(args_val) = arguments {
            if !local_names.iter().any(|n| n == "arguments") {
                self.set_property(env, "arguments", args_val);
            }
        }
        if let (Some(fn_name), Some(val)) = (callee_name, callee_val) {
            if !local_names.iter().any(|n| n == &fn_name) {
                self.set_property(env, &fn_name, val);
            }
        }
        if let Some(frame) = self.frames.last_mut() {
            frame.env_obj = Some(env);
        }
        upvalues.push(env);
        upvalues
    }
}
