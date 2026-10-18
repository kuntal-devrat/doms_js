use super::super::Compiler;
use super::unwrap_grouping;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

impl Compiler {
    pub(crate) fn compile_assignment(
        &mut self,
        target: &str,
        value: &Expression,
    ) -> Result<(), CompileError> {
        let inferred_name = if let Some(stripped) = target.strip_prefix("@name:") {
            let parts: Vec<&str> = stripped.split(':').collect();
            if !parts.is_empty() {
                Some(parts[0])
            } else {
                None
            }
        } else if !target.starts_with('@') {
            Some(target)
        } else {
            None
        };

        match value {
            Expression::Conditional { condition, then_expr, else_expr } if inferred_name.is_some() => {
                self.compile_conditional_with_name(condition, then_expr, else_expr, inferred_name)?;
            }
            Expression::ClassExpr(class_def) if inferred_name.is_some() && class_def.name.is_none() => {
                self.compile_class_definition_with_name(class_def, inferred_name.map(|s| s.to_string()))?;
            }
            _ => {
                self.compile_expression(value)?;
                if let Some(name) = inferred_name {
                    if Self::is_anonymous_function_definition(value) {
                        let name_idx = self.chunk.add_constant(Constant::String(name.to_string()));
                        self.chunk.emit(OpCode::SetFunctionName(name_idx), 0);
                    }
                }
            }
        }
        self.chunk.emit(OpCode::Dup, 0);
        if self.with_depth == 0 {
            if let Some(slot) = self.symbols.resolve_local(target) {
                self.chunk.emit(OpCode::SetLocal(slot), 0);
                self.chunk.emit(OpCode::Pop, 0);
                return Ok(());
            }
        }
        let name_idx = self.chunk.add_constant(Constant::String(target.to_string()));
        self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
        self.chunk.emit(OpCode::Pop, 0);
        Ok(())
    }

    pub(crate) fn compile_member_assignment(
        &mut self,
        object: &Expression,
        property: &str,
        value: &Expression,
    ) -> Result<(), CompileError> {
        if matches!(object, Expression::Super) {
            self.compile_expression(value)?;
            let prop_idx = self.chunk.add_constant(Constant::String(property.to_string()));
            self.chunk.emit(OpCode::SetSuperProp(prop_idx), 0);
        } else {
            self.compile_expression(object)?;
            self.compile_expression(value)?;
            let prop_idx = self.chunk.add_constant(Constant::String(property.to_string()));
            self.chunk.emit(OpCode::SetProp(prop_idx), 0);
        }
        Ok(())
    }

    pub(crate) fn compile_index_assignment(
        &mut self,
        object: &Expression,
        index: &Expression,
        value: &Expression,
    ) -> Result<(), CompileError> {
        if matches!(object, Expression::Super) {
            self.compile_expression(index)?;
            self.compile_expression(value)?;
            self.chunk.emit(OpCode::SetSuperElem, 0);
        } else {
            self.compile_expression(object)?;
            self.compile_expression(index)?;
            self.compile_expression(value)?;
            self.chunk.emit(OpCode::SetElem, 0);
        }
        Ok(())
    }

    pub(crate) fn compile_update(
        &mut self,
        op: &UpdateOp,
        is_prefix: bool,
        expr: &Expression,
    ) -> Result<(), CompileError> {
        let delta_is_pos = *op == UpdateOp::Inc;
        let one_idx = self.chunk.add_constant(Constant::Number(1.0));
        match unwrap_grouping(expr) {
            Expression::Identifier(name) => {
                if self.with_depth == 0 {
                    if let Some(slot) = self.symbols.resolve_local(name) {
                        self.chunk.emit(OpCode::GetLocal(slot), 0);
                    } else {
                        let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                        self.chunk.emit(OpCode::GetGlobal(name_idx), 0);
                    }
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::GetGlobal(name_idx), 0);
                }
                self.chunk.emit(OpCode::Pos, 0);

                if is_prefix {
                    self.chunk.emit(OpCode::LoadConst(one_idx), 0);
                    if delta_is_pos {
                        self.chunk.emit(OpCode::Add, 0);
                    } else {
                        self.chunk.emit(OpCode::Sub, 0);
                    }
                    self.chunk.emit(OpCode::Dup, 0);
                    if self.with_depth == 0 {
                        if let Some(slot) = self.symbols.resolve_local(name) {
                            self.chunk.emit(OpCode::SetLocal(slot), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        } else {
                            let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                            self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        }
                    } else {
                        let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                        self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                    }
                } else {
                    self.chunk.emit(OpCode::Dup, 0);
                    self.chunk.emit(OpCode::LoadConst(one_idx), 0);
                    if delta_is_pos {
                        self.chunk.emit(OpCode::Add, 0);
                    } else {
                        self.chunk.emit(OpCode::Sub, 0);
                    }
                    if self.with_depth == 0 {
                        if let Some(slot) = self.symbols.resolve_local(name) {
                            self.chunk.emit(OpCode::SetLocal(slot), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        } else {
                            let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                            self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        }
                    } else {
                        let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                        self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                    }
                }
            }
            Expression::Member { object, property } => {
                let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                if matches!(object.as_ref(), Expression::Super) {
                    self.chunk.emit(OpCode::LoadSuper, 0);
                    self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                    self.chunk.emit(OpCode::Pos, 0);
                    if is_prefix {
                        self.chunk.emit(OpCode::LoadConst(one_idx), 0);
                        if delta_is_pos {
                            self.chunk.emit(OpCode::Add, 0);
                        } else {
                            self.chunk.emit(OpCode::Sub, 0);
                        }
                        self.chunk.emit(OpCode::SetSuperProp(prop_idx), 0);
                    } else {
                        self.chunk.emit(OpCode::Dup, 0);
                        self.chunk.emit(OpCode::LoadConst(one_idx), 0);
                        if delta_is_pos {
                            self.chunk.emit(OpCode::Add, 0);
                        } else {
                            self.chunk.emit(OpCode::Sub, 0);
                        }
                        self.chunk.emit(OpCode::SetSuperProp(prop_idx), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                    }
                } else {
                    self.compile_expression(object)?;
                    self.chunk.emit(OpCode::Dup, 0);
                    self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                    self.chunk.emit(OpCode::Pos, 0);
                    if is_prefix {
                        self.chunk.emit(OpCode::LoadConst(one_idx), 0);
                        if delta_is_pos {
                            self.chunk.emit(OpCode::Add, 0);
                        } else {
                            self.chunk.emit(OpCode::Sub, 0);
                        }
                        self.chunk.emit(OpCode::SetProp(prop_idx), 0);
                    } else {
                        self.chunk.emit(OpCode::LoadConst(one_idx), 0);
                        if delta_is_pos {
                            self.chunk.emit(OpCode::Add, 0);
                        } else {
                            self.chunk.emit(OpCode::Sub, 0);
                        }
                        self.chunk.emit(OpCode::SetProp(prop_idx), 0);
                    }
                }
            }
            _ => return Err(CompileError::General("Invalid assignment target in update expression".into())),
        }
        Ok(())
    }

    pub(crate) fn compile_destructuring_assignment(
        &mut self,
        pattern: &Expression,
        value: &Expression,
    ) -> Result<(), CompileError> {
        self.compile_expression(value)?;
        self.chunk.emit(OpCode::Dup, 0);
        let tmp_var = format!("@dstr_val_{}", self.symbols.locals.len());
        let slot = self.symbols.define_local(tmp_var.clone());
        self.chunk.emit(OpCode::SetLocal(slot), 0);
        self.chunk.emit(OpCode::Pop, 0);

        let mut parser = doms_parser::Parser::new("")
            .map_err(|e| CompileError::General(format!("{:?}", e)))?;
        let mut stmts = Vec::new();
        parser.desugar_assignment_destructuring(
            pattern,
            Expression::Identifier(tmp_var),
            &mut stmts,
        );

        let prev_internal = self.internal_statements;
        self.internal_statements = true;
        for stmt in &stmts {
            self.compile_statement(stmt, false)?;
        }
        self.internal_statements = prev_internal;

        Ok(())
    }
}
