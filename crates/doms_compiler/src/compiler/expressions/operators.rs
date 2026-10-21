use super::super::Compiler;
use super::unwrap_grouping;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

impl Compiler {
    pub(crate) fn compile_binary(
        &mut self,
        op: &BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> Result<(), CompileError> {
        match op {
            BinaryOp::LogicalAnd => {
                self.compile_expression(left)?;
                self.chunk.emit(OpCode::Dup, 0);
                let jump_idx = self.chunk.emit(OpCode::JumpIfFalse(0), 0);
                self.chunk.emit(OpCode::Pop, 0);
                self.compile_expression(right)?;
                let target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_idx, target);
                return Ok(());
            }
            BinaryOp::LogicalOr => {
                self.compile_expression(left)?;
                self.chunk.emit(OpCode::Dup, 0);
                let jump_idx = self.chunk.emit(OpCode::JumpIfTrue(0), 0);
                self.chunk.emit(OpCode::Pop, 0);
                self.compile_expression(right)?;
                let target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_idx, target);
                return Ok(());
            }
            BinaryOp::NullishCoalescing => {
                self.compile_expression(left)?;
                self.chunk.emit(OpCode::Dup, 0);
                self.chunk.emit(OpCode::LoadNull, 0);
                self.chunk.emit(OpCode::StrictEq, 0);
                let jump_is_null = self.chunk.emit(OpCode::JumpIfTrue(0), 0);
                self.chunk.emit(OpCode::Dup, 0);
                self.chunk.emit(OpCode::LoadUndefined, 0);
                self.chunk.emit(OpCode::StrictEq, 0);
                let jump_is_undef = self.chunk.emit(OpCode::JumpIfTrue(0), 0);
                let jump_has_val = self.chunk.emit(OpCode::Jump(0), 0);

                let nullish_target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_is_null, nullish_target);
                self.chunk.patch_jump(jump_is_undef, nullish_target);
                self.chunk.emit(OpCode::Pop, 0);
                self.compile_expression(right)?;

                let end_target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_has_val, end_target);
                return Ok(());
            }
            BinaryOp::In => {
                if let Expression::PrivateIdentifier(name) = unwrap_grouping(left) {
                    self.compile_expression(right)?;
                    let const_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::PrivateIn(const_idx), 0);
                    return Ok(());
                }
            }
            _ => {}
        }

        self.compile_expression(left)?;
        self.compile_expression(right)?;

        match op {
            BinaryOp::Add => self.chunk.emit(OpCode::Add, 0),
            BinaryOp::Sub => self.chunk.emit(OpCode::Sub, 0),
            BinaryOp::Mul => self.chunk.emit(OpCode::Mul, 0),
            BinaryOp::Div => self.chunk.emit(OpCode::Div, 0),
            BinaryOp::Mod => self.chunk.emit(OpCode::Mod, 0),
            BinaryOp::Exp => self.chunk.emit(OpCode::Exp, 0),
            BinaryOp::Eq => self.chunk.emit(OpCode::Eq, 0),
            BinaryOp::NotEq => self.chunk.emit(OpCode::NotEq, 0),
            BinaryOp::StrictEq => self.chunk.emit(OpCode::StrictEq, 0),
            BinaryOp::StrictNotEq => self.chunk.emit(OpCode::StrictNotEq, 0),
            BinaryOp::Lt => self.chunk.emit(OpCode::Lt, 0),
            BinaryOp::Lte => self.chunk.emit(OpCode::Lte, 0),
            BinaryOp::Gt => self.chunk.emit(OpCode::Gt, 0),
            BinaryOp::Gte => self.chunk.emit(OpCode::Gte, 0),
            BinaryOp::Instanceof => self.chunk.emit(OpCode::Instanceof, 0),
            BinaryOp::In => self.chunk.emit(OpCode::In, 0),
            BinaryOp::BitwiseAnd => self.chunk.emit(OpCode::BitAnd, 0),
            BinaryOp::BitwiseOr => self.chunk.emit(OpCode::BitOr, 0),
            BinaryOp::BitwiseXor => self.chunk.emit(OpCode::BitXor, 0),
            BinaryOp::Shl => self.chunk.emit(OpCode::Shl, 0),
            BinaryOp::Shr => self.chunk.emit(OpCode::Shr, 0),
            BinaryOp::Ushr => self.chunk.emit(OpCode::Ushr, 0),
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing => {
                unreachable!()
            }
        };
        Ok(())
    }

    pub(crate) fn compile_unary(
        &mut self,
        op: &UnaryOp,
        expr: &Expression,
    ) -> Result<(), CompileError> {
        if *op == UnaryOp::Typeof {
            if let Expression::Identifier(name) = unwrap_grouping(expr) {
                if self.is_in_function && self.with_depth == 0 {
                    if let Some(slot) = self.symbols.resolve_local(name) {
                        self.chunk.emit(OpCode::GetLocal(slot), 0);
                        self.chunk.emit(OpCode::Typeof, 0);
                        return Ok(());
                    }
                }
                let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::GetGlobalSafe(name_idx), 0);
                self.chunk.emit(OpCode::Typeof, 0);
                return Ok(());
            }
        }
        if *op == UnaryOp::Delete {
            match unwrap_grouping(expr) {
                Expression::Member { object, property } => {
                    if matches!(object.as_ref(), Expression::Super) {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::DeleteSuper, 0);
                    } else {
                        self.compile_expression(object)?;
                        let name_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::DeleteProp(name_idx), 0);
                    }
                }
                Expression::Index { object, index } => {
                    if matches!(object.as_ref(), Expression::Super) {
                        self.compile_expression(object)?;
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::DeleteSuper, 0);
                    } else {
                        self.compile_expression(object)?;
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::DeleteElem, 0);
                    }
                }
                Expression::Identifier(name) => {
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::DeleteIdent(name_idx), 0);
                }
                _ => {
                    self.compile_expression(expr)?;
                    self.chunk.emit(OpCode::Delete, 0);
                }
            }
            return Ok(());
        }
        self.compile_expression(expr)?;
        match op {
            UnaryOp::Pos => self.chunk.emit(OpCode::Pos, 0),
            UnaryOp::Neg => self.chunk.emit(OpCode::Neg, 0),
            UnaryOp::Not => self.chunk.emit(OpCode::Not, 0),
            UnaryOp::BitNot => self.chunk.emit(OpCode::BitNot, 0),
            UnaryOp::Typeof => self.chunk.emit(OpCode::Typeof, 0),
            UnaryOp::Void => self.chunk.emit(OpCode::Void, 0),
            UnaryOp::Delete => unreachable!(),
        };
        Ok(())
    }

    pub(crate) fn compile_conditional(
        &mut self,
        condition: &Expression,
        then_expr: &Expression,
        else_expr: &Expression,
    ) -> Result<(), CompileError> {
        self.compile_conditional_with_name(condition, then_expr, else_expr, None)
    }

    pub(crate) fn compile_conditional_with_name(
        &mut self,
        condition: &Expression,
        then_expr: &Expression,
        else_expr: &Expression,
        inferred_name: Option<&str>,
    ) -> Result<(), CompileError> {
        self.compile_expression(condition)?;
        let jump_false_idx = self.chunk.emit(OpCode::JumpIfFalse(0), 0);
        self.compile_expression(then_expr)?;
        if let Some(name) = inferred_name {
            if Self::is_anonymous_function_definition(then_expr) {
                let name_idx = self.chunk.add_constant(Constant::String(name.to_string()));
                self.chunk.emit(OpCode::SetFunctionName(name_idx), 0);
            }
        }
        let jump_exit_idx = self.chunk.emit(OpCode::Jump(0), 0);
        let else_start = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(jump_false_idx, else_start);
        self.compile_expression(else_expr)?;
        let exit_target = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(jump_exit_idx, exit_target);
        Ok(())
    }
}
