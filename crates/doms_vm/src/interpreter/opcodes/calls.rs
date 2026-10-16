//! Function call and object construction dispatch arms.

use super::*;

impl Vm {
    pub(crate) fn op_call(&mut self, argc: u8, target_depth: usize) -> Result<(), VmError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();
        let callee = self.pop()?;
        match self.execute_call_internal(callee, JsValue::undefined(), args) {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {},
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
            }
        }
        Ok(())
    }

    pub(crate) fn op_call_with_this(&mut self, argc: u8, target_depth: usize) -> Result<(), VmError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();
        let callee = self.pop()?;
        let this_val = self.pop()?;
        match self.execute_call_internal(callee, this_val, args) {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {},
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
        match self.execute_call_internal(callee, JsValue::undefined(), args) {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {},
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
        match self.execute_call_internal(callee, this_val, args) {
            Ok(Some(res)) => self.push(res),
            Ok(None) => {},
            Err(err) => {
                self.handle_exception_at_depth(err, target_depth)?;
            }
        }
        Ok(())
    }

    pub(crate) fn op_construct(&mut self, argc: u8, target_depth: usize) -> Result<(), VmError> {
        let mut args = Vec::with_capacity(argc as usize);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();
        let callee = self.pop()?;
        self.execute_construct(callee, args, target_depth)?;
        Ok(())
    }

    pub(crate) fn op_construct_spread(&mut self, target_depth: usize) -> Result<(), VmError> {
        let args_val = self.pop()?;
        let callee = self.pop()?;
        let args = self.array_to_vec_spread(args_val);
        self.execute_construct(callee, args, target_depth)?;
        Ok(())
    }
}
