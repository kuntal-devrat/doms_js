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

        let (local_names, callee_name) = {
            let frame = self.frames.last().unwrap();
            let c_name = frame.callee_func.as_ref().and_then(|f| f.name.clone());
            (frame.local_names.clone(), c_name)
        };
        let prev_temp = self.temp_roots.len();
        let env = self.alloc_object();
        self.temp_roots.push(env);
        if let Some(frame) = self.frames.last_mut() {
            frame.env_obj = Some(env);
        }
        for (i, name) in local_names.iter().enumerate() {
            let cur_env = self.temp_roots[prev_temp];
            let val = self.frames.last().and_then(|f| f.locals.get(i).copied()).unwrap_or(JsValue::undefined());
            self.set_property(cur_env, name, val);
        }
        if self.frames.last().and_then(|f| f.arguments).is_some() {
            if !local_names.iter().any(|n| n == "arguments") {
                let cur_env = self.temp_roots[prev_temp];
                if let Some(args_val) = self.frames.last().and_then(|f| f.arguments) {
                    self.set_property(cur_env, "arguments", args_val);
                }
            }
        }
        if let Some(fn_name) = callee_name {
            if !local_names.iter().any(|n| n == &fn_name) {
                let cur_env = self.temp_roots[prev_temp];
                if let Some(val) = self.frames.last().and_then(|f| f.callee_val) {
                    self.set_property(cur_env, &fn_name, val);
                }
            }
        }
        let fresh_env = self.temp_roots[prev_temp];
        self.temp_roots.truncate(prev_temp);
        if let Some(frame) = self.frames.last_mut() {
            frame.env_obj = Some(fresh_env);
        }
        upvalues.push(fresh_env);
        upvalues
    }
}
