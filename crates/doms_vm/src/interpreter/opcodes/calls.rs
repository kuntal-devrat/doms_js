//! Function call and object construction dispatch arms.

use super::*;

impl Vm {
    pub(crate) fn op_call(&mut self, argc: u8, target_depth: usize) -> Result<(), VmError> {
        let argc_usize = argc as usize;
        let stack_len = self.stack.len();
        if stack_len < argc_usize + 1 {
            return Err(VmError::TypeError("Stack underflow in op_call".into()));
        }
        let callee = self.stack[stack_len - argc_usize - 1];
        let stack_args_start = stack_len - argc_usize;
        let res = self.execute_call_from_stack_at(
            callee,
            JsValue::undefined(),
            stack_args_start,
            argc_usize,
        );
        self.stack.truncate(stack_len - argc_usize - 1);
        match res {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {}
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
            }
        }
        Ok(())
    }

    pub(crate) fn op_call_with_this(
        &mut self,
        argc: u8,
        target_depth: usize,
    ) -> Result<(), VmError> {
        let argc_usize = argc as usize;
        let stack_len = self.stack.len();
        if stack_len < argc_usize + 2 {
            return Err(VmError::TypeError("Stack underflow in op_call_with_this".into()));
        }
        let this_val = self.stack[stack_len - argc_usize - 2];
        let callee = self.stack[stack_len - argc_usize - 1];
        let stack_args_start = stack_len - argc_usize;
        let res = self.execute_call_from_stack_at(callee, this_val, stack_args_start, argc_usize);
        self.stack.truncate(stack_len - argc_usize - 2);
        match res {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {}
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
            }
        }
        Ok(())
    }

    pub(crate) fn op_call_spread(&mut self, target_depth: usize) -> Result<(), VmError> {
        let args_val = self.pop()?;
        let callee = self.pop()?;
        let args = self.array_to_vec_spread(args_val);
        match self.execute_call_internal(callee, JsValue::undefined(), &args) {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {}
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
            }
        }
        Ok(())
    }

    pub(crate) fn op_call_with_this_spread(&mut self, target_depth: usize) -> Result<(), VmError> {
        let args_val = self.pop()?;
        let callee = self.pop()?;
        let this_val = self.pop()?;
        let args = self.array_to_vec_spread(args_val);
        match self.execute_call_internal(callee, this_val, &args) {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {}
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
            }
        }
        Ok(())
    }

    pub(crate) fn op_construct(&mut self, argc: u8, target_depth: usize) -> Result<(), VmError> {
        let argc_usize = argc as usize;
        let stack_len = self.stack.len();
        if stack_len < argc_usize + 1 {
            return Err(VmError::TypeError("Stack underflow in op_construct".into()));
        }
        let callee = self.stack[stack_len - argc_usize - 1];
        let stack_args_start = stack_len - argc_usize;
        let prev_temp = self.temp_roots.len();
        self.temp_roots.push(callee);
        let args_start = self.temp_roots.len();
        self.temp_roots.extend_from_slice(&self.stack[stack_args_start..stack_len]);
        self.stack.truncate(stack_len - argc_usize - 1);
        let cur_callee = self.temp_roots[prev_temp];
        let res = if argc_usize <= 8 {
            let mut stack_args = [JsValue::undefined(); 8];
            stack_args[..argc_usize]
                .copy_from_slice(&self.temp_roots[args_start..args_start + argc_usize]);
            let r = self.construct(cur_callee, &stack_args[..argc_usize]);
            stack_args = [JsValue::undefined(); 8];
            std::hint::black_box(&stack_args);
            r
        } else {
            let heap_args = self.temp_roots[args_start..args_start + argc_usize].to_vec();
            self.construct(cur_callee, &heap_args)
        };
        self.temp_roots.truncate(prev_temp);
        match res {
            Ok(val) => {
                self.push(val);
                Ok(())
            }
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                Ok(())
            }
        }
    }

    pub(crate) fn op_construct_spread(&mut self, target_depth: usize) -> Result<(), VmError> {
        let args_val = self.pop()?;
        let callee = self.pop()?;
        let args = self.array_to_vec_spread(args_val);
        self.execute_construct(callee, &args, target_depth)?;
        Ok(())
    }

    pub(crate) fn op_super_call(
        &mut self,
        args: &[JsValue],
        target_depth: usize,
    ) -> Result<(), VmError> {
        let ctor_frame_idx = match self.frames.iter().rposition(|f| !f.is_arrow) {
            Some(i) => i,
            None => {
                self.handle_exception_at_depth(
                    VmError::ReferenceError("super() called outside of constructor".into()),
                    target_depth,
                )?;
                return Ok(());
            }
        };

        let callee_val = match self.frames[ctor_frame_idx].callee_val {
            Some(v) => v,
            None => {
                self.handle_exception_at_depth(
                    VmError::TypeError("Super constructor is not a constructor".into()),
                    target_depth,
                )?;
                return Ok(());
            }
        };

        let super_ctor = if let Some(callee_ptr) = callee_val.as_object_ptr() {
            let callee_obj = unsafe { &*(callee_ptr as *mut JsObject) };
            callee_obj.prototype.unwrap_or(JsValue::null())
        } else {
            JsValue::null()
        };

        let is_valid_super = self.is_constructor(super_ctor) || {
            if let Some(sym_ctor) = self.globals.get("Symbol") {
                super_ctor == *sym_ctor
            } else {
                false
            }
        };
        if !is_valid_super {
            self.handle_exception_at_depth(
                VmError::TypeError("Super constructor is not a constructor".into()),
                target_depth,
            )?;
            return Ok(());
        }

        let new_target = self.frames[ctor_frame_idx].new_target.unwrap_or(callee_val);

        let res = self.construct_with_new_target(super_ctor, args, new_target);
        match res {
            Ok(instance) => {
                if self.frames[ctor_frame_idx].this_val != JsValue::uninitialized() {
                    self.handle_exception_at_depth(
                        VmError::ReferenceError("Super constructor may only be called once".into()),
                        target_depth,
                    )?;
                    return Ok(());
                }
                for f in &mut self.frames[ctor_frame_idx..] {
                    if f.is_arrow || f.this_val == JsValue::uninitialized() {
                        f.this_val = instance;
                    }
                }
                self.push(instance);
                Ok(())
            }
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
                Ok(())
            }
        }
    }
}
