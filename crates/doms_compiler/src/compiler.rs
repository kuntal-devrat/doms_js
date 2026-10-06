//! AST to bytecode compiler.

use crate::bytecode::{BytecodeChunk, Constant, OpCode};
use crate::scope::SymbolTable;
use doms_parser::ast::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Variable '{0}' already declared in this scope")]
    DuplicateVariable(String),
    #[error("Too many constants in chunk")]
    TooManyConstants,
    #[error("Too many locals")]
    TooManyLocals,
    #[error("Compile error: {0}")]
    General(String),
}

pub struct Compiler {
    chunk: BytecodeChunk,
    symbols: SymbolTable,
}

impl Compiler {
    pub fn new() -> Self {
        Self { chunk: BytecodeChunk::new(), symbols: SymbolTable::new() }
    }

    pub fn compile(mut self, program: &Program) -> Result<BytecodeChunk, CompileError> {
        let total = program.body.len();
        for (i, stmt) in program.body.iter().enumerate() {
            let is_last = i == total - 1;
            self.compile_statement(stmt, is_last)?;
        }
        self.chunk.emit(OpCode::Halt, 0);
        Ok(self.chunk)
    }

    fn compile_statement(&mut self, stmt: &Statement, is_last: bool) -> Result<(), CompileError> {
        match stmt {
            Statement::Expression(expr) => {
                self.compile_expression(expr)?;
                if !is_last {
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::VarDecl { name, init, .. } => {
                if let Some(init_expr) = init {
                    self.compile_expression(init_expr)?;
                } else {
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }

                if self.symbols.scope_depth > 0 {
                    let slot = self.symbols.define_local(name.clone());
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::Block(stmts) => {
                self.symbols.enter_scope();
                let len = stmts.len();
                for (idx, s) in stmts.iter().enumerate() {
                    self.compile_statement(s, is_last && idx == len - 1)?;
                }
                let popped = self.symbols.exit_scope();
                for _ in 0..popped {
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::If { condition, then_branch, else_branch } => {
                self.compile_expression(condition)?;
                let jump_false_idx = self.chunk.emit(OpCode::JumpIfFalse(0), 0);

                self.compile_statement(then_branch, is_last)?;

                if let Some(else_stmt) = else_branch {
                    let jump_exit_idx = self.chunk.emit(OpCode::Jump(0), 0);
                    let else_start = self.chunk.instructions.len() as i32;
                    self.chunk.patch_jump(jump_false_idx, else_start);

                    self.compile_statement(else_stmt, is_last)?;
                    let exit_target = self.chunk.instructions.len() as i32;
                    self.chunk.patch_jump(jump_exit_idx, exit_target);
                } else {
                    let exit_target = self.chunk.instructions.len() as i32;
                    self.chunk.patch_jump(jump_false_idx, exit_target);
                }
            }
            Statement::While { condition, body } => {
                let loop_start = self.chunk.instructions.len() as i32;
                self.compile_expression(condition)?;
                let jump_exit_idx = self.chunk.emit(OpCode::JumpIfFalse(0), 0);

                self.compile_statement(body, false)?;
                self.chunk.emit(OpCode::Jump(loop_start), 0);

                let exit_target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_exit_idx, exit_target);
            }
            Statement::Return(opt_expr) => {
                if let Some(expr) = opt_expr {
                    self.compile_expression(expr)?;
                } else {
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }
                self.chunk.emit(OpCode::Return, 0);
            }
            Statement::FunctionDecl { .. } => {
                // Function declaration: stored for closures/calls in Phase 1
            }
            Statement::Empty => {}
        }
        Ok(())
    }

    fn compile_expression(&mut self, expr: &Expression) -> Result<(), CompileError> {
        match expr {
            Expression::Literal(lit) => match lit {
                Literal::Number(n) => {
                    let idx = self.chunk.add_constant(Constant::Number(*n));
                    self.chunk.emit(OpCode::LoadConst(idx), 0);
                }
                Literal::String(s) => {
                    let idx = self.chunk.add_constant(Constant::String(s.clone()));
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
            },
            Expression::Identifier(name) => {
                if let Some(slot) = self.symbols.resolve_local(name) {
                    self.chunk.emit(OpCode::GetLocal(slot), 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::GetGlobal(name_idx), 0);
                }
            }
            Expression::Assignment { target, value } => {
                self.compile_expression(value)?;
                self.chunk.emit(OpCode::Dup, 0); // Leave value on stack as expression result
                if let Some(slot) = self.symbols.resolve_local(target) {
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(target.clone()));
                    self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Expression::Binary { op, left, right } => {
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
                    BinaryOp::Eq => self.chunk.emit(OpCode::Eq, 0),
                    BinaryOp::NotEq => self.chunk.emit(OpCode::NotEq, 0),
                    BinaryOp::StrictEq => self.chunk.emit(OpCode::StrictEq, 0),
                    BinaryOp::StrictNotEq => self.chunk.emit(OpCode::StrictNotEq, 0),
                    BinaryOp::Lt => self.chunk.emit(OpCode::Lt, 0),
                    BinaryOp::Lte => self.chunk.emit(OpCode::Lte, 0),
                    BinaryOp::Gt => self.chunk.emit(OpCode::Gt, 0),
                    BinaryOp::Gte => self.chunk.emit(OpCode::Gte, 0),
                    BinaryOp::BitwiseAnd => self.chunk.emit(OpCode::BitAnd, 0),
                    BinaryOp::BitwiseOr => self.chunk.emit(OpCode::BitOr, 0),
                    BinaryOp::BitwiseXor => self.chunk.emit(OpCode::BitXor, 0),
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => unreachable!(),
                };
            }
            Expression::Unary { op, expr } => {
                self.compile_expression(expr)?;
                match op {
                    UnaryOp::Neg => self.chunk.emit(OpCode::Neg, 0),
                    UnaryOp::Not => self.chunk.emit(OpCode::Not, 0),
                    UnaryOp::BitNot => self.chunk.emit(OpCode::BitNot, 0),
                    UnaryOp::Typeof => self.chunk.emit(OpCode::Typeof, 0),
                };
            }
            Expression::Call { callee, args } => {
                self.compile_expression(callee)?;
                for arg in args {
                    self.compile_expression(arg)?;
                }
                self.chunk.emit(OpCode::Call(args.len() as u8), 0);
            }
            Expression::Member { object, property } => {
                self.compile_expression(object)?;
                let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                self.chunk.emit(OpCode::LoadConst(prop_idx), 0);
                // Member lookup opcode in VM
            }
        }
        Ok(())
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doms_parser::parse;

    #[test]
    fn test_compile_arithmetic() {
        let prog = parse("let x = 1 + 2 * 3;").unwrap();
        let chunk = Compiler::new().compile(&prog).unwrap();
        assert!(chunk.instructions.contains(&OpCode::Add));
        assert!(chunk.instructions.contains(&OpCode::Mul));
    }
}
