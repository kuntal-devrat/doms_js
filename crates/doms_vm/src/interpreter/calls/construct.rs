use super::*;

impl Vm {
    pub fn construct(&mut self, callee: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
        self.construct_with_new_target(callee, args, callee)
    }

    pub fn construct_with_new_target(
        &mut self,
        callee: JsValue,
        args: &[JsValue],
        new_target: JsValue,
    ) -> Result<JsValue, VmError> {
        if !self.is_constructor(callee) {
            return Err(VmError::TypeError(format!("{} is not a constructor", self.to_string_value(callee))));
        }
        if !new_target.is_undefined() && !self.is_constructor(new_target) {
            return Err(VmError::TypeError(format!("{} is not a constructor", self.to_string_value(new_target))));
        }

        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(callee);
        self.temp_roots.push(new_target);
        let args_start = self.temp_roots.len();
        self.temp_roots.extend_from_slice(args);
        let args_len = args.len();

        let cur_callee = self.temp_roots[prev_temp];
        if let Some(ptr) = cur_callee.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Proxy { .. } => {
                    let cur_callee = self.temp_roots[prev_temp];
                    let cur_nt = self.temp_roots[prev_temp + 1];
                    let cur_args = self.temp_roots[args_start..args_start + args_len].to_vec();
                    self.temp_roots.truncate(prev_temp);
                    self.proxy_construct(cur_callee, &cur_args, cur_nt)
                }
                ObjectKind::NativeFunction(native) => {
                    let native = native.clone();
                    let cur_callee = self.temp_roots[prev_temp];
                    let cur_nt = self.temp_roots[prev_temp + 1];
                    let new_target_val = if cur_nt.is_undefined() { cur_callee } else { cur_nt };
                    let proto = self.get_property(new_target_val, "prototype");
                    self.temp_roots.push(proto);
                    let this_obj = self.alloc_object();
                    self.temp_roots.push(this_obj);
                    let cur_proto = self.temp_roots[args_start + args_len];
                    let cur_this = self.temp_roots[args_start + args_len + 1];
                    if cur_proto.is_object() {
                        if let Some(this_ptr) = cur_this.as_object_ptr() {
                            let jsobj = unsafe { &mut *(this_ptr as *mut JsObject) };
                            jsobj.prototype = Some(cur_proto);
                        }
                    }
                    let cur_this = self.temp_roots[args_start + args_len + 1];
                    let cur_callee = self.temp_roots[prev_temp];
                    let cur_nt = self.temp_roots[prev_temp + 1];
                    let new_target_val = if cur_nt.is_undefined() { cur_callee } else { cur_nt };
                    let prev_constructing = self.is_constructing;
                    let prev_nt = self.current_new_target;
                    self.is_constructing = true;
                    self.current_new_target = Some(new_target_val);
                    let res = if args_len <= 8 {
                        let mut stack_args = [JsValue::undefined(); 8];
                        stack_args[..args_len].copy_from_slice(&self.temp_roots[args_start..args_start + args_len]);
                        native.call(self, cur_this, &stack_args[..args_len])
                    } else {
                        let cur_args = self.temp_roots[args_start..args_start + args_len].to_vec();
                        native.call(self, cur_this, &cur_args)
                    };
                    self.is_constructing = prev_constructing;
                    self.current_new_target = prev_nt;
                    let val = res?;
                    let cur_proto = self.temp_roots[args_start + args_len];
                    let cur_this = self.temp_roots[args_start + args_len + 1];
                    let cur_callee = self.temp_roots[prev_temp];
                    let cur_nt = self.temp_roots[prev_temp + 1];
                    let new_target_val = if cur_nt.is_undefined() { cur_callee } else { cur_nt };
                    let ret = if val.is_object() {
                        if cur_proto.is_object() && new_target_val != cur_callee {
                            if let Some(val_ptr) = val.as_object_ptr() {
                                let jsobj = unsafe { &mut *(val_ptr as *mut JsObject) };
                                jsobj.prototype = Some(cur_proto);
                            }
                        }
                        val
                    } else {
                        cur_this
                    };
                    self.temp_roots.truncate(prev_temp);
                    Ok(ret)
                }
                ObjectKind::Function(f) => {
                    let cur_callee = self.temp_roots[prev_temp];
                    let cur_nt = self.temp_roots[prev_temp + 1];
                    let new_target_val = if cur_nt.is_undefined() { cur_callee } else { cur_nt };
                    let proto = if new_target_val == cur_callee {
                        f.prototype.unwrap_or_else(|| self.get_property(new_target_val, "prototype"))
                    } else {
                        self.get_property(new_target_val, "prototype")
                    };
                    self.temp_roots.push(proto);
                    let shape_id = if new_target_val == cur_callee {
                        f.initial_shape.unwrap_or(ShapeRegistry::ROOT_SHAPE_ID)
                    } else {
                        ShapeRegistry::ROOT_SHAPE_ID
                    };
                    let this_obj = self.alloc_object_with_shape_and_proto(shape_id, if proto.is_object() { Some(proto) } else { None });
                    self.temp_roots.push(this_obj);
                    let cur_callee = self.temp_roots[prev_temp];
                    let cur_this = self.temp_roots[args_start + args_len + 1];
                    let prev_constructing = self.is_constructing;
                    let prev_nt = self.current_new_target;
                    self.is_constructing = true;
                    self.current_new_target = Some(new_target_val);
                    let call_res = if args_len <= 8 {
                        let mut stack_args = [JsValue::undefined(); 8];
                        stack_args[..args_len].copy_from_slice(&self.temp_roots[args_start..args_start + args_len]);
                        self.execute_call_internal(cur_callee, cur_this, &stack_args[..args_len])
                    } else {
                        let cur_args = self.temp_roots[args_start..args_start + args_len].to_vec();
                        self.execute_call_internal(cur_callee, cur_this, &cur_args)
                    };
                    let res_opt = call_res?;
                    let val = if let Some(res) = res_opt {
                        res
                    } else {
                        let target_depth = self.frames.len() - 1;
                        self.run_until_depth(target_depth)?
                    };
                    self.is_constructing = prev_constructing;
                    self.current_new_target = prev_nt;
                    let cur_this = self.temp_roots[args_start + args_len + 1];
                    let cur_callee = self.temp_roots[prev_temp];
                    let cur_nt = self.temp_roots[prev_temp + 1];
                    let new_target_val = if cur_nt.is_undefined() { cur_callee } else { cur_nt };

                    if new_target_val == cur_callee && !val.is_object() {
                        if let Some(this_ptr) = cur_this.as_object_ptr() {
                            let this_shape = unsafe { (*(this_ptr as *mut JsObject)).shape_id };
                            if this_shape != ShapeRegistry::ROOT_SHAPE_ID {
                                if let Some(callee_ptr) = cur_callee.as_object_ptr() {
                                    let callee_obj = unsafe { &mut *(callee_ptr as *mut JsObject) };
                                    if let ObjectKind::Function(ref mut callee_func) = callee_obj.kind {
                                        callee_func.initial_shape = Some(this_shape);
                                        let chunk_id = callee_func.chunk.id;
                                        for (ip, op) in callee_func.chunk.instructions.iter().enumerate() {
                                            if let doms_compiler::bytecode::OpCode::SetProp(prop_idx) = op {
                                                if let doms_compiler::bytecode::Constant::String(name) = &callee_func.chunk.constants[*prop_idx as usize] {
                                                    if let Some(offset) = self.shapes.lookup_offset(this_shape, name) {
                                                        callee_func.chunk.record_feedback_shape(ip, this_shape, offset);
                                                        let ics = self.chunk_ics.entry(chunk_id).or_insert_with(|| {
                                                            vec![crate::inline_cache::InlineCacheSlot::new(); callee_func.chunk.instructions.len()]
                                                        });
                                                        if ip < ics.len() {
                                                            ics[ip].state = crate::inline_cache::IcState::Monomorphic;
                                                            ics[ip].name_idx = *prop_idx;
                                                            ics[ip].entries[0] = crate::inline_cache::ShapeOffsetPair { shape_id: this_shape, offset };
                                                            ics[ip].entry_count = 1;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        callee_func.jit_attempted = true;
                                        callee_func.jit_entry = crate::jit::JitCompiler::compile_native(&callee_func.chunk);
                                    }
                                }
                            }
                        }
                    }

                    self.temp_roots.truncate(prev_temp);
                    if val.is_object() {
                        Ok(val)
                    } else {
                        Ok(cur_this)
                    }
                }
                _ => {
                    let cur_callee = self.temp_roots[prev_temp];
                    self.temp_roots.truncate(prev_temp);
                    Err(VmError::TypeError(format!("{} is not a constructor", self.to_string_value(cur_callee))))
                }
            }
        } else {
            self.temp_roots.truncate(prev_temp);
            Err(VmError::TypeError("Target is not a constructor".into()))
        }
    }

    pub fn execute_construct(
        &mut self,
        callee: JsValue,
        args: &[JsValue],
        target_depth: usize,
    ) -> Result<(), VmError> {
        match self.construct(callee, args) {
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

    pub fn is_array_result(&self, val: JsValue) -> Result<bool, VmError> {
        let mut curr = val;
        let mut hops = 0;
        while let Some(ptr) = curr.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            match &obj.kind {
                ObjectKind::Array(_) => return Ok(true),
                ObjectKind::Proxy { target, handler } => {
                    if handler.is_null() {
                        return Err(VmError::TypeError("Cannot perform IsArray on a revoked proxy".into()));
                    }
                    hops += 1;
                    if hops > 100 {
                        return Ok(false);
                    }
                    curr = *target;
                }
                _ => return Ok(false),
            }
        }
        Ok(false)
    }

    pub fn is_array(&self, val: JsValue) -> bool {
        self.is_array_result(val).unwrap_or(false)
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
            if length > 4294967295 {
                return Err(VmError::RangeError("Invalid array length".into()));
            }
            let cap = length.min(100_000);
            return Ok(self.alloc_array(Vec::with_capacity(cap)));
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
            if length > 4294967295 {
                return Err(VmError::RangeError("Invalid array length".into()));
            }
            let cap = length.min(100_000);
            return Ok(self.alloc_array(Vec::with_capacity(cap)));
        }
        if !self.is_constructor(ctor) {
            return Err(VmError::TypeError("Species is not a constructor".into()));
        }
        let len_val = if length <= i32::MAX as usize {
            JsValue::from_int32(length as i32)
        } else {
            JsValue::from_double(length as f64)
        };
        self.construct(ctor, &[len_val])
    }
}

