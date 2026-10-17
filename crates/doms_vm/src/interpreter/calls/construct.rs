use super::*;

impl Vm {
    pub fn construct(&mut self, callee: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        if !self.is_constructor(callee) {
            return Err(VmError::TypeError(format!("{} is not a constructor", self.to_string_value(callee))));
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
                    self.temp_roots.extend_from_slice(args);
                    let prev_constructing = self.is_constructing;
                    self.is_constructing = true;
                    let res = native.call(self, this_obj, args);
                    self.is_constructing = prev_constructing;
                    self.temp_roots.truncate(prev_temp);
                    let val = res?;
                    if val.is_object() {
                        Ok(val)
                    } else {
                        Ok(this_obj)
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
                    let call_res = self.execute_call_internal(callee, this_obj, args.to_vec());
                    self.is_constructing = prev_constructing;
                    let res_opt = call_res?;
                    let val = if let Some(res) = res_opt {
                        res
                    } else {
                        let target_depth = self.frames.len() - 1;
                        self.run_until_depth(target_depth)?
                    };
                    if val.is_object() {
                        Ok(val)
                    } else {
                        Ok(this_obj)
                    }
                }
                _ => Err(VmError::TypeError(format!("{} is not a constructor", self.to_string_value(callee)))),
            }
        } else {
            Err(VmError::TypeError("Target is not a constructor".into()))
        }
    }

    pub fn execute_construct(
        &mut self,
        callee: JsValue,
        args: Vec<JsValue>,
        target_depth: usize,
    ) -> Result<(), VmError> {
        match self.construct(callee, &args) {
            Ok(res) => {
                self.push(res);
                Ok(())
            }
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                Ok(())
            }
        }
    }

    pub fn is_array(&self, val: JsValue) -> bool {
        let mut curr = val;
        let mut hops = 0;
        while let Some(ptr) = curr.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Array(_) => return true,
                ObjectKind::Proxy { target, .. } => {
                    hops += 1;
                    if hops > 100 {
                        return false;
                    }
                    curr = *target;
                }
                _ => return false,
            }
        }
        false
    }

    #[inline]
    pub fn is_js_object(&self, val: JsValue) -> bool {
        if !val.is_object() {
            return false;
        }
        if let Some(ptr) = val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            !matches!(obj.kind, ObjectKind::String(_) | ObjectKind::BigInt(_) | ObjectKind::Symbol(_))
        } else {
            false
        }
    }

    pub fn array_species_create(&mut self, original: JsValue, length: usize) -> Result<JsValue, VmError> {
        if !self.is_array(original) {
            return Ok(self.alloc_array(Vec::with_capacity(length)));
        }
        let mut ctor = self.get_property_result(original, "constructor")?;
        if self.is_constructor(ctor) {
            if let Some(&array_ctor) = self.globals.get("Array") {
                if ctor == array_ctor {
                    ctor = JsValue::undefined();
                }
            }
        }
        if self.is_js_object(ctor) {
            let sp = if let Some(sym_species) = self.symbol_species {
                self.get_symbol_property_result(ctor, sym_species)?
            } else {
                JsValue::undefined()
            };
            if sp.is_null() {
                ctor = JsValue::undefined();
            } else {
                ctor = sp;
            }
        }
        if ctor.is_undefined() {
            return Ok(self.alloc_array(Vec::with_capacity(length)));
        }
        if !self.is_constructor(ctor) {
            return Err(VmError::TypeError("Species is not a constructor".into()));
        }
        self.construct(ctor, &[JsValue::from_int32(length as i32)])
    }
}

