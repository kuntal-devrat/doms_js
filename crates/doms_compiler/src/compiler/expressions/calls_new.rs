use super::super::Compiler;
use super::unwrap_grouping;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

impl Compiler {
    pub(crate) fn compile_spread_args(&mut self, args: &[Expression]) -> Result<(), CompileError> {
        self.chunk.emit(OpCode::CreateArray(0), 0);
        for arg in args {
            match arg {
                Expression::Spread(expr) => {
                    self.compile_expression(expr)?;
                    self.chunk.emit(OpCode::ArraySpreadPush, 0);
                }
                _ => {
                    self.compile_expression(arg)?;
                    self.chunk.emit(OpCode::ArrayPush, 0);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn compile_call(
        &mut self,
        callee: &Expression,
        args: &[Expression],
    ) -> Result<(), CompileError> {
        let has_spread = args.iter().any(|a| matches!(a, Expression::Spread(_)));
        let unwrapped = unwrap_grouping(callee);
        if has_spread {
            match unwrapped {
                Expression::Member { object, property } => {
                    if matches!(object.as_ref(), Expression::Super) {
                        self.chunk.emit(OpCode::This, 0);
                        self.chunk.emit(OpCode::LoadSuper, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                    } else {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                    }
                }
                Expression::Index { object, index } => {
                    if matches!(object.as_ref(), Expression::Super) {
                        self.chunk.emit(OpCode::This, 0);
                        self.chunk.emit(OpCode::LoadSuper, 0);
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                    } else {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                    }
                }
                Expression::Super => {
                    self.compile_spread_args(args)?;
                    self.chunk.emit(OpCode::SuperCallSpread, 0);
                }
                Expression::OptionalChain(oc)
                    if oc
                        .chain
                        .last()
                        .map_or(false, |e| matches!(e, ChainElement::Property { .. })) =>
                {
                    if let Some(ChainElement::Property { name, optional }) = oc.chain.last() {
                        let prefix_oc = OptionalChain {
                            base: oc.base.clone(),
                            chain: oc.chain[..oc.chain.len() - 1].to_vec(),
                        };
                        if prefix_oc.chain.is_empty() {
                            self.compile_expression(&prefix_oc.base)?;
                        } else {
                            self.compile_optional_chain(&prefix_oc)?;
                        }
                        if *optional {
                            let j = self.emit_nullish_short_circuit(1);
                            self.chunk.emit(OpCode::Dup, 0);
                            let j_skip = self.chunk.emit(OpCode::Jump(0), 0);
                            let target_valid = self.chunk.instructions.len() as i32;
                            self.chunk.patch_jump(j, target_valid);
                            self.chunk.emit(OpCode::Dup, 0);
                            let prop_idx = self.chunk.add_constant(Constant::String(name.clone()));
                            self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                            let target_after = self.chunk.instructions.len() as i32;
                            self.chunk.patch_jump(j_skip, target_after);
                        } else {
                            self.chunk.emit(OpCode::Dup, 0);
                            let prop_idx = self.chunk.add_constant(Constant::String(name.clone()));
                            self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                        }
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                    }
                }
                _ => {
                    self.compile_expression(callee)?;
                    self.compile_spread_args(args)?;
                    self.chunk.emit(OpCode::CallSpread, 0);
                }
            }
        } else {
            match unwrapped {
                Expression::Member { object, property } => {
                    if matches!(object.as_ref(), Expression::Super) {
                        self.chunk.emit(OpCode::This, 0);
                        self.chunk.emit(OpCode::LoadSuper, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    } else {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    }
                }
                Expression::Index { object, index } => {
                    if matches!(object.as_ref(), Expression::Super) {
                        self.chunk.emit(OpCode::This, 0);
                        self.chunk.emit(OpCode::LoadSuper, 0);
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    } else {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    }
                }
                Expression::Super => {
                    for arg in args {
                        self.compile_expression(arg)?;
                    }
                    self.chunk.emit(OpCode::SuperCall(args.len() as u8), 0);
                }
                Expression::OptionalChain(oc)
                    if oc.chain.last().map_or(false, |e| {
                        matches!(e, ChainElement::Property { .. } | ChainElement::Index { .. })
                    }) =>
                {
                    match oc.chain.last().unwrap() {
                        ChainElement::Property { name, optional } => {
                            let prefix_oc = OptionalChain {
                                base: oc.base.clone(),
                                chain: oc.chain[..oc.chain.len() - 1].to_vec(),
                            };
                            if prefix_oc.chain.is_empty() {
                                self.compile_expression(&prefix_oc.base)?;
                            } else {
                                self.compile_optional_chain(&prefix_oc)?;
                            }
                            if *optional {
                                let j_nullish = self.emit_nullish_short_circuit(1);
                                self.chunk.emit(OpCode::Dup, 0);
                                let prop_idx =
                                    self.chunk.add_constant(Constant::String(name.clone()));
                                self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                                let j_after = self.chunk.emit(OpCode::Jump(0), 0);

                                let target_nullish = self.chunk.instructions.len() as i32;
                                self.chunk.patch_jump(j_nullish, target_nullish);
                                self.chunk.emit(OpCode::Dup, 0);

                                let target_after = self.chunk.instructions.len() as i32;
                                self.chunk.patch_jump(j_after, target_after);
                            } else {
                                self.chunk.emit(OpCode::Dup, 0);
                                let prop_idx =
                                    self.chunk.add_constant(Constant::String(name.clone()));
                                self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                            }
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        }
                        ChainElement::Index { index, optional } => {
                            let prefix_oc = OptionalChain {
                                base: oc.base.clone(),
                                chain: oc.chain[..oc.chain.len() - 1].to_vec(),
                            };
                            if prefix_oc.chain.is_empty() {
                                self.compile_expression(&prefix_oc.base)?;
                            } else {
                                self.compile_optional_chain(&prefix_oc)?;
                            }
                            if *optional {
                                let j_nullish = self.emit_nullish_short_circuit(1);
                                self.chunk.emit(OpCode::Dup, 0);
                                self.compile_expression(index)?;
                                self.chunk.emit(OpCode::GetElem, 0);
                                let j_after = self.chunk.emit(OpCode::Jump(0), 0);

                                let target_nullish = self.chunk.instructions.len() as i32;
                                self.chunk.patch_jump(j_nullish, target_nullish);
                                self.chunk.emit(OpCode::Dup, 0);

                                let target_after = self.chunk.instructions.len() as i32;
                                self.chunk.patch_jump(j_after, target_after);
                            } else {
                                self.chunk.emit(OpCode::Dup, 0);
                                self.compile_expression(index)?;
                                self.chunk.emit(OpCode::GetElem, 0);
                            }
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        }
                        _ => unreachable!(),
                    }
                }
                _ => {
                    let is_direct_eval =
                        matches!(callee, Expression::Identifier(name) if name == "eval");
                    self.compile_expression(callee)?;
                    for arg in args {
                        self.compile_expression(arg)?;
                    }
                    if is_direct_eval {
                        self.chunk.emit(OpCode::CallDirectEval(args.len() as u8), 0);
                    } else {
                        self.chunk.emit(OpCode::Call(args.len() as u8), 0);
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn compile_new(
        &mut self,
        callee: &Expression,
        args: &[Expression],
    ) -> Result<(), CompileError> {
        if args.iter().any(|a| matches!(a, Expression::Spread(_))) {
            self.compile_expression(callee)?;
            self.compile_spread_args(args)?;
            self.chunk.emit(OpCode::ConstructSpread, 0);
        } else {
            self.compile_expression(callee)?;
            for arg in args {
                self.compile_expression(arg)?;
            }
            self.chunk.emit(OpCode::Construct(args.len() as u8), 0);
        }
        Ok(())
    }

    pub(crate) fn compile_tagged_template(
        &mut self,
        tag: &Expression,
        quasis: &[Option<String>],
        raw_quasis: &[String],
        exprs: &[Expression],
    ) -> Result<(), CompileError> {
        let unwrapped = unwrap_grouping(tag);
        let has_this = match unwrapped {
            Expression::Member { object, property } => {
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
                true
            }
            Expression::Index { object, index } => {
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
                true
            }
            _ => {
                self.compile_expression(tag)?;
                false
            }
        };

        // Construct template object using GetTemplateObject opcode
        static TEMPLATE_SITE_GEN: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(1);
        let site = TEMPLATE_SITE_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let const_idx = self.chunk.add_constant(Constant::TemplateObject {
            site,
            quasis: quasis.to_vec(),
            raw: raw_quasis.to_vec(),
        });
        self.chunk.emit(OpCode::GetTemplateObject(const_idx), 0);

        // Compile expressions
        for expr in exprs {
            self.compile_expression(expr)?;
        }

        let total_args = (1 + exprs.len()) as u8;
        if has_this {
            self.chunk.emit(OpCode::CallWithThis(total_args), 0);
        } else {
            self.chunk.emit(OpCode::Call(total_args), 0);
        }

        Ok(())
    }
}
