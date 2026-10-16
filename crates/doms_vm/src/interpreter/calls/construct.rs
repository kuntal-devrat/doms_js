use super::*;

impl Vm {
    pub fn execute_construct(
        &mut self,
        callee: JsValue,
        args: Vec<JsValue>,
        target_depth: usize,
    ) -> Result<(), VmError> {
        if !self.is_constructor(callee) {
            let msg = format!("{} is not a constructor", self.to_string_value(callee));
            self.handle_exception_at_depth(VmError::TypeError(msg), target_depth)?;
            return Ok(());
        }

        if let Some(ptr) = callee.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::NativeFunction(native) => {
                    let proto = self.get_property(callee, "prototype");
                    let this_obj = self.alloc_object();
                    if proto.is_object() {
                        let this_ptr = this_obj.as_object_ptr().unwrap();
                        let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
                        jsobj.prototype = Some(proto);
                    }
                    let prev_temp = self.temp_roots.len();
                    self.temp_roots.extend_from_slice(&args);
                    let prev_constructing = self.is_constructing;
                    self.is_constructing = true;
                    let res = native.call(self, this_obj, &args);
                    self.is_constructing = prev_constructing;
                    self.temp_roots.truncate(prev_temp);
                    match res {
                        Ok(res) => {
                            if res.is_object() {
                                self.push(res);
                            } else {
                                self.push(this_obj);
                            }
                        }
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
                    }
                }
                ObjectKind::Function(_) => {
                    let this_obj = self.alloc_object();
                    let proto = self.get_property(callee, "prototype");
                    if proto.is_object() {
                        let this_ptr = this_obj.as_object_ptr().unwrap();
                        let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
                        jsobj.prototype = Some(proto);
                    }
                    let prev_constructing = self.is_constructing;
                    self.is_constructing = true;
                    let res = self.call(callee, this_obj, &args);
                    self.is_constructing = prev_constructing;
                    match res {
                        Ok(res) => {
                            if res.is_object() {
                                self.push(res);
                            } else {
                                self.push(this_obj);
                            }
                        }
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                        }
                    }
                }
                _ => {
                    let err = VmError::TypeError(format!("{} is not a constructor", self.to_string_value(callee)));
                    self.handle_exception_at_depth(err, target_depth)?;
                }
            }
        } else {
            let err = VmError::TypeError("Target is not a constructor".into());
            self.handle_exception_at_depth(err, target_depth)?;
        }
        Ok(())
    }
}
