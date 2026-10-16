use super::super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

impl Compiler {
    pub(crate) fn compile_literal(&mut self, lit: &Literal) -> Result<(), CompileError> {
        match lit {
            Literal::Number(n) => {
                let idx = self.chunk.add_constant(Constant::Number(*n));
                self.chunk.emit(OpCode::LoadConst(idx), 0);
            }
            Literal::String { value, .. } => {
                let idx = self.chunk.add_constant(Constant::String(value.clone()));
                self.chunk.emit(OpCode::LoadConst(idx), 0);
            }
            Literal::Boolean(true) => {
                self.chunk.emit(OpCode::LoadTrue, 0);
            }
            Literal::Boolean(false) => {
                self.chunk.emit(OpCode::LoadFalse, 0);
            }
            Literal::Null => {
                self.chunk.emit(OpCode::LoadNull, 0);
            }
            Literal::Undefined => {
                self.chunk.emit(OpCode::LoadUndefined, 0);
            }
            Literal::Hole => {
                self.chunk.emit(OpCode::LoadHole, 0);
            }
            Literal::RegExp { pattern, flags } => {
                let p_idx = self.chunk.add_constant(Constant::String(pattern.clone()));
                let f_idx = self.chunk.add_constant(Constant::String(flags.clone()));
                self.chunk.emit(OpCode::CreateRegExp(p_idx, f_idx), 0);
            }
            Literal::BigInt(bi) => {
                let idx = self.chunk.add_constant(Constant::BigInt(bi.clone()));
                self.chunk.emit(OpCode::LoadConst(idx), 0);
            }
        }
        Ok(())
    }

    pub(crate) fn compile_identifier(&mut self, name: &str) -> Result<(), CompileError> {
        if self.is_in_function && self.with_depth == 0 {
            if let Some(slot) = self.symbols.resolve_local(name) {
                self.chunk.emit(OpCode::GetLocal(slot), 0);
                return Ok(());
            }
        }
        let name_idx = self.chunk.add_constant(Constant::String(name.to_string()));
        self.chunk.emit(OpCode::GetGlobal(name_idx), 0);
        Ok(())
    }

    pub(crate) fn compile_object(&mut self, props: &[ObjectProperty]) -> Result<(), CompileError> {
        self.chunk.emit(OpCode::CreateObject, 0);
        for prop in props {
            match prop {
                ObjectProperty::KeyValue(key, val_expr) => {
                    match key {
                        PropertyKey::Literal(k) => {
                            self.chunk.emit(OpCode::Dup, 0);
                            self.compile_expression(val_expr)?;
                            let prop_idx = self.chunk.add_constant(Constant::String(k.clone()));
                            self.chunk.emit(OpCode::SetProp(prop_idx), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        }
                        PropertyKey::Computed(k_expr) => {
                            self.chunk.emit(OpCode::Dup, 0);
                            self.compile_expression(k_expr)?;
                            self.compile_expression(val_expr)?;
                            self.chunk.emit(OpCode::SetElem, 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        }
                    }
                }
                ObjectProperty::Getter(key, fn_expr) => {
                    match key {
                        PropertyKey::Literal(k) => {
                            self.chunk.emit(OpCode::Dup, 0);
                            self.compile_expression(fn_expr)?;
                            let prop_idx = self.chunk.add_constant(Constant::String(k.clone()));
                            self.chunk.emit(OpCode::DefineGetter(prop_idx), 0);
                        }
                        PropertyKey::Computed(k_expr) => {
                            self.chunk.emit(OpCode::Dup, 0);
                            self.compile_expression(k_expr)?;
                            self.compile_expression(fn_expr)?;
                            self.chunk.emit(OpCode::DefineComputedGetter, 0);
                        }
                    }
                }
                ObjectProperty::Setter(key, fn_expr) => {
                    match key {
                        PropertyKey::Literal(k) => {
                            self.chunk.emit(OpCode::Dup, 0);
                            self.compile_expression(fn_expr)?;
                            let prop_idx = self.chunk.add_constant(Constant::String(k.clone()));
                            self.chunk.emit(OpCode::DefineSetter(prop_idx), 0);
                        }
                        PropertyKey::Computed(k_expr) => {
                            self.chunk.emit(OpCode::Dup, 0);
                            self.compile_expression(k_expr)?;
                            self.compile_expression(fn_expr)?;
                            self.chunk.emit(OpCode::DefineComputedSetter, 0);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn compile_array(&mut self, elements: &[Expression]) -> Result<(), CompileError> {
        if elements.iter().any(|el| matches!(el, Expression::Spread(_))) {
            self.compile_spread_args(elements)?;
        } else {
            for el in elements {
                self.compile_expression(el)?;
            }
            self.chunk.emit(OpCode::CreateArray(elements.len() as u16), 0);
        }
        Ok(())
    }
}
