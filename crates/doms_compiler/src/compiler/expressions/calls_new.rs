use super::super::Compiler;
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
        if has_spread {
            match callee {
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
                    if let Some(slot) = self.symbols.resolve_local("super") {
                        self.chunk.emit(OpCode::GetLocal(slot), 0);
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallSpread, 0);
                        self.chunk.emit(OpCode::SetThis, 0);
                    } else {
                        self.chunk.emit(OpCode::This, 0);
                        self.chunk.emit(OpCode::LoadSuper, 0);
                        self.compile_spread_args(args)?;
                        self.chunk.emit(OpCode::CallWithThisSpread, 0);
                        self.chunk.emit(OpCode::SetThis, 0);
                    }
                }
                _ => {
                    self.compile_expression(callee)?;
                    self.compile_spread_args(args)?;
                    self.chunk.emit(OpCode::CallSpread, 0);
                }
            }
        } else {
            match callee {
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
                    if let Some(slot) = self.symbols.resolve_local("super") {
                        self.chunk.emit(OpCode::GetLocal(slot), 0);
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::Call(args.len() as u8), 0);
                        self.chunk.emit(OpCode::SetThis, 0);
                    } else {
                        self.chunk.emit(OpCode::This, 0);
                        self.chunk.emit(OpCode::LoadSuper, 0);
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        self.chunk.emit(OpCode::SetThis, 0);
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
}
