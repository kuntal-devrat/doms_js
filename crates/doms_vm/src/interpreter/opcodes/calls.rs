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
            self.construct(cur_callee, &stack_args[..argc_usize])
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
}
