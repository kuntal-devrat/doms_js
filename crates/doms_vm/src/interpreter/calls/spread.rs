use super::*;

impl Vm {
    pub fn array_to_vec_spread(&self, val: JsValue) -> Vec<JsValue> {
        if let Some(ptr) = val.as_object_ptr() {
            let jsobj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(elements) = &jsobj.kind {
                return elements.clone();
            }
        }
        Vec::new()
    }

    pub fn to_spread_args(&mut self, val: JsValue) -> Result<Vec<JsValue>, VmError> {
        if let Some(sym_iter) = self.symbol_iterator {
            if let Ok(method) = self.get_symbol_property_result(val, sym_iter) {
                if !method.is_undefined() && !method.is_null() {
                    if !self.is_callable(method) {
                        return Err(VmError::TypeError("Symbol.iterator is not callable".into()));
                    }
                    let it = self.call(method, val, &[])?;
                    let next_fn = match self.get_property_result(it, "next") {
                        Ok(n) => n,
                        Err(e) => return Err(e),
                    };
                    if !self.is_callable(next_fn) {
                        return Err(VmError::TypeError("iterator.next is not callable".into()));
                    }
                    let prev_temp = self.temp_roots.len();
                    self.temp_roots.push(it);
                    self.temp_roots.push(next_fn);
                    let mut res = Vec::new();
                    loop {
                        if let Some(d) = self.deadline {
                            if Instant::now() > d {
                                self.temp_roots.truncate(prev_temp);
                                return Err(VmError::RuntimeError(
                                    "Execution timeout exceeded".into(),
                                ));
                            }
                        }
                        if res.len() >= 1_000_000 {
                            self.temp_roots.truncate(prev_temp);
                            return Err(VmError::RangeError(
                                "Spread iterable exceeds maximum size".into(),
                            ));
                        }
                        let cur_it = self.temp_roots[prev_temp];
                        let cur_next = self.temp_roots[prev_temp + 1];
                        let step = match self.call(cur_next, cur_it, &[]) {
                            Ok(s) => s,
                            Err(e) => {
                                self.temp_roots.truncate(prev_temp);
                                return Err(e);
                            }
                        };
                        if !step.is_js_object() {
                            self.temp_roots.truncate(prev_temp);
                            return Err(VmError::TypeError(
                                "Iterator result is not an object".into(),
                            ));
                        }
                        self.temp_roots.push(step);
                        let cur_step = self.temp_roots[self.temp_roots.len() - 1];
                        let done = match self.get_property_result(cur_step, "done") {
                            Ok(d) => d,
                            Err(e) => {
                                self.temp_roots.truncate(prev_temp);
                                return Err(e);
                            }
                        };
                        if done.is_truthy() {
                            self.temp_roots.pop();
                            break;
                        }
                        let cur_step = self.temp_roots[self.temp_roots.len() - 1];
                        let v = match self.get_property_result(cur_step, "value") {
                            Ok(v) => v,
                            Err(e) => {
                                self.temp_roots.truncate(prev_temp);
                                return Err(e);
                            }
                        };
                        self.temp_roots.pop();
                        self.temp_roots.push(v);
                        res.push(v);
                    }
                    for (i, val) in self.temp_roots[prev_temp + 2..].iter().enumerate() {
                        res[i] = *val;
                    }
                    self.temp_roots.truncate(prev_temp);
                    return Ok(res);
                }
            }
        }

        if val.is_string() {
            let s = self.to_string_value(val);
            return Ok(s.chars().map(|c| self.alloc_string(c.to_string())).collect());
        }

        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::Array(arr) = &obj.kind {
                return Ok(arr.clone());
            }
        }

        Err(VmError::TypeError(format!("{} is not iterable", self.to_string_value(val))))
    }
}
