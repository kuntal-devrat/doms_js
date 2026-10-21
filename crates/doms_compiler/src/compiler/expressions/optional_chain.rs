use super::super::Compiler;
use super::unwrap_grouping;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

impl Compiler {
    /// Emits bytecode checking if the value on top of the stack is null or undefined.
    /// If null or undefined:
    /// Pops `pop_count` values, pushes `JsValue::undefined()`, and jumps to an end target.
    /// Returns the jump instruction index to be patched later.
    pub(crate) fn emit_nullish_short_circuit(&mut self, pop_count: usize) -> usize {
        self.chunk.emit(OpCode::Dup, 0);
        self.chunk.emit(OpCode::LoadNull, 0);
        self.chunk.emit(OpCode::StrictEq, 0);
        let jump_null = self.chunk.emit(OpCode::JumpIfTrue(0), 0);

        self.chunk.emit(OpCode::Dup, 0);
        self.chunk.emit(OpCode::LoadUndefined, 0);
        self.chunk.emit(OpCode::StrictEq, 0);
        let jump_undef = self.chunk.emit(OpCode::JumpIfTrue(0), 0);

        let jump_continue = self.chunk.emit(OpCode::Jump(0), 0);

        // Nullish target
        let nullish_target = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(jump_null, nullish_target);
        self.chunk.patch_jump(jump_undef, nullish_target);

        for _ in 0..pop_count {
            self.chunk.emit(OpCode::Pop, 0);
        }
        self.chunk.emit(OpCode::LoadUndefined, 0);
        let jump_to_end = self.chunk.emit(OpCode::Jump(0), 0);

        // Continue target
        let continue_target = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(jump_continue, continue_target);

        jump_to_end
    }

    pub(crate) fn compile_optional_chain(
        &mut self,
        oc: &OptionalChain,
    ) -> Result<(), CompileError> {
        let mut jumps_to_end = Vec::new();

        // Check special case: base is Member or Index and chain starts with Call
        // e.g. a.b?.() or a[b]?.()
        let unwrapped_base = unwrap_grouping(&oc.base);
        let mut start_idx = 0;

        match unwrapped_base {
            Expression::Member { object, property }
                if !oc.chain.is_empty() && matches!(oc.chain[0], ChainElement::Call { .. }) =>
            {
                let call_elem = &oc.chain[0];
                if let ChainElement::Call { args, optional } = call_elem {
                    if matches!(object.as_ref(), Expression::Super) {
                        self.chunk.emit(OpCode::This, 0);
                        self.chunk.emit(OpCode::LoadSuper, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                    } else {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                    }
                    if *optional {
                        let j = self.emit_nullish_short_circuit(2);
                        jumps_to_end.push(j);
                    }
                    let has_spread = args.iter().any(|a| matches!(a, Expression::Spread(_)));
                    if has_spread {
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                    } else {
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    }
                    start_idx = 1;
                }
            }
            Expression::Index { object, index }
                if !oc.chain.is_empty() && matches!(oc.chain[0], ChainElement::Call { .. }) =>
            {
                let call_elem = &oc.chain[0];
                if let ChainElement::Call { args, optional } = call_elem {
                    if matches!(object.as_ref(), Expression::Super) {
                        self.chunk.emit(OpCode::This, 0);
                        self.chunk.emit(OpCode::LoadSuper, 0);
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);
                    } else {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);
                    }
                    if *optional {
                        let j = self.emit_nullish_short_circuit(2);
                        jumps_to_end.push(j);
                    }
                    let has_spread = args.iter().any(|a| matches!(a, Expression::Spread(_)));
                    if has_spread {
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                    } else {
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    }
                    start_idx = 1;
                }
            }
            Expression::OptionalChain(inner)
                if !oc.chain.is_empty()
                    && matches!(oc.chain[0], ChainElement::Call { .. })
                    && inner.chain.last().map_or(false, |e| {
                        matches!(e, ChainElement::Property { .. } | ChainElement::Index { .. })
                    }) =>
            {
                let call_elem = &oc.chain[0];
                if let ChainElement::Call { args, optional: call_opt } = call_elem {
                    let prefix_inner = OptionalChain {
                        base: inner.base.clone(),
                        chain: inner.chain[..inner.chain.len() - 1].to_vec(),
                    };
                    if prefix_inner.chain.is_empty() {
                        self.compile_expression(&prefix_inner.base)?;
                    } else {
                        self.compile_optional_chain(&prefix_inner)?;
                    }

                    match inner.chain.last().unwrap() {
                        ChainElement::Property { name, optional: prop_opt } => {
                            if *prop_opt {
                                let j = self.emit_nullish_short_circuit(1);
                                jumps_to_end.push(j);
                            }
                            self.chunk.emit(OpCode::Dup, 0);
                            let prop_idx = self.chunk.add_constant(Constant::String(name.clone()));
                            self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                        }
                        ChainElement::Index { index, optional: idx_opt } => {
                            if *idx_opt {
                                let j = self.emit_nullish_short_circuit(1);
                                jumps_to_end.push(j);
                            }
                            self.chunk.emit(OpCode::Dup, 0);
                            self.compile_expression(index)?;
                            self.chunk.emit(OpCode::GetElem, 0);
                        }
                        _ => unreachable!(),
                    }

                    if *call_opt {
                        let j = self.emit_nullish_short_circuit(2);
                        jumps_to_end.push(j);
                    }
                    let has_spread = args.iter().any(|a| matches!(a, Expression::Spread(_)));
                    if has_spread {
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                    } else {
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    }
                    start_idx = 1;
                }
            }
            _ => {
                self.compile_expression(&oc.base)?;
            }
        }

        let mut i = start_idx;
        while i < oc.chain.len() {
            let elem = &oc.chain[i];
            let next_elem = oc.chain.get(i + 1);

            match elem {
                ChainElement::Property { name, optional } => {
                    if let Some(ChainElement::Call { args, optional: call_opt }) = next_elem {
                        if *optional {
                            let j = self.emit_nullish_short_circuit(1);
                            jumps_to_end.push(j);
                        }
                        self.chunk.emit(OpCode::Dup, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(name.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);

                        if *call_opt {
                            let j = self.emit_nullish_short_circuit(2);
                            jumps_to_end.push(j);
                        }
                        let has_spread = args.iter().any(|a| matches!(a, Expression::Spread(_)));
                        if has_spread {
                            self.compile_spread_args(args)?;
                            self.chunk.emit(OpCode::CallWithThisSpread, 0);
                        } else {
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        }
                        i += 2;
                        continue;
                    } else {
                        if *optional {
                            let j = self.emit_nullish_short_circuit(1);
                            jumps_to_end.push(j);
                        }
                        let prop_idx = self.chunk.add_constant(Constant::String(name.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                    }
                }
                ChainElement::Index { index, optional } => {
                    if let Some(ChainElement::Call { args, optional: call_opt }) = next_elem {
                        if *optional {
                            let j = self.emit_nullish_short_circuit(1);
                            jumps_to_end.push(j);
                        }
                        self.chunk.emit(OpCode::Dup, 0);
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);

                        if *call_opt {
                            let j = self.emit_nullish_short_circuit(2);
                            jumps_to_end.push(j);
                        }
                        let has_spread = args.iter().any(|a| matches!(a, Expression::Spread(_)));
                        if has_spread {
                            self.compile_spread_args(args)?;
                            self.chunk.emit(OpCode::CallWithThisSpread, 0);
                        } else {
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        }
                        i += 2;
                        continue;
                    } else {
                        if *optional {
                            let j = self.emit_nullish_short_circuit(1);
                            jumps_to_end.push(j);
                        }
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);
                    }
                }
                ChainElement::Call { args, optional } => {
                    if *optional {
                        let j = self.emit_nullish_short_circuit(1);
                        jumps_to_end.push(j);
                    }
                    let has_spread = args.iter().any(|a| matches!(a, Expression::Spread(_)));
                    if has_spread {
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallSpread, 0);
                    } else {
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::Call(args.len() as u8), 0);
                    }
                }
            }
            i += 1;
        }

        let end_target = self.chunk.instructions.len() as i32;
        for j in jumps_to_end {
            self.chunk.patch_jump(j, end_target);
        }

        Ok(())
    }
}
