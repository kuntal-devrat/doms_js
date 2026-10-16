//! Class element definition (`DefineClassElement`).

use super::*;

impl Vm {

    pub(crate) fn op_define_class_element(
        &mut self,
        flags: u8,
        target_depth: usize,
    ) -> Result<(), VmError> {
                    let is_static = (flags & 1) != 0;
                    let kind = flags & 0xFE; // 0 = method, 2 = getter, 4 = setter
                    let func_val = self.pop()?;
                    let key_val = self.pop()?;
                    let ctor_val = self.pop()?;

                    let prop_key = match self.to_property_key(key_val) {
                        Ok(k) => k,
                        Err(err) => {
                            self.handle_exception_at_depth(err, target_depth)?;
                            return Ok(());
                        }
                    };

                    if is_static {
                        if let PropertyKeyVal::String(ref s) = prop_key {
                            if s == "prototype" {
                                let err = VmError::TypeError("Cannot define static property named prototype".to_string());
                                self.handle_exception_at_depth(err, target_depth)?;
                                return Ok(());
                            }
                        }
                    }

                    let target = if is_static {
                        ctor_val
                    } else {
                        self.get_property(ctor_val, "prototype")
                    };

                    if let Some(target_ptr) = target.as_object_ptr() {
                        let target_obj = unsafe { &mut *(target_ptr as *mut JsObject) };
                        match kind {
                            2 => { // Getter
                                match prop_key {
                                    PropertyKeyVal::String(s) => {
                                        self.non_enumerable_props.insert((target, s.clone()));
                                        let accs = target_obj.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                                        accs.entry(s).or_insert(Accessor { getter: None, setter: None }).getter = Some(func_val);
                                    }
                                    PropertyKeyVal::Symbol(sym) => {
                                        target_obj.insert_symbol_accessor_getter(sym, func_val);
                                    }
                                }
                            }
                            4 => { // Setter
                                match prop_key {
                                    PropertyKeyVal::String(s) => {
                                        self.non_enumerable_props.insert((target, s.clone()));
                                        let accs = target_obj.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
                                        accs.entry(s).or_insert(Accessor { getter: None, setter: None }).setter = Some(func_val);
                                    }
                                    PropertyKeyVal::Symbol(sym) => {
                                        target_obj.insert_symbol_accessor_setter(sym, func_val);
                                    }
                                }
                            }
                            _ => { // Method
                                match prop_key {
                                    PropertyKeyVal::String(s) => {
                                        self.non_enumerable_props.insert((target, s.clone()));
                                        self.set_property(target, &s, func_val);
                                    }
                                    PropertyKeyVal::Symbol(sym) => {
                                        target_obj.insert_symbol_property(sym, func_val);
                                    }
                                }
                            }
                        }
                    }
        Ok(())
    }
}
