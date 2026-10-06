//! AST to bytecode compiler.

use crate::bytecode::{BytecodeChunk, Constant, FunctionDef, OpCode};
use crate::scope::SymbolTable;
use doms_parser::ast::*;
use std::sync::Arc;
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
    pub chunk: BytecodeChunk,
    pub symbols: SymbolTable,
    pub break_targets: Vec<Vec<usize>>,
    pub continue_targets: Vec<Vec<usize>>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            chunk: BytecodeChunk::new(),
            symbols: SymbolTable::new(),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
        }
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

    pub fn compile_function(
        name: Option<String>,
        params: Vec<String>,
        body: &[Statement],
    ) -> Result<FunctionDef, CompileError> {
        let mut compiler = Compiler::new();
        compiler.symbols.enter_scope();
        for p in &params {
            compiler.symbols.define_local(p.clone());
        }
        let total = body.len();
        for (i, stmt) in body.iter().enumerate() {
            let is_last = i == total - 1;
            compiler.compile_statement(stmt, is_last)?;
        }
        // Default return undefined
        compiler.chunk.emit(OpCode::LoadUndefined, 0);
        compiler.chunk.emit(OpCode::Return, 0);
        Ok(FunctionDef { name, params, chunk: compiler.chunk })
    }

    fn compile_statement(&mut self, stmt: &Statement, is_last: bool) -> Result<(), CompileError> {
        match stmt {
            Statement::Expression(expr) => {
                self.compile_expression(expr)?;
                if !is_last {
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::VarDecl { declarations, .. } => {
                for decl in declarations {
                    if let Some(init_expr) = &decl.init {
                        self.compile_expression(init_expr)?;
                    } else {
                        self.chunk.emit(OpCode::LoadUndefined, 0);
                    }

                    if self.symbols.scope_depth > 0 {
                        self.symbols.define_local(decl.name.clone());
                    } else {
                        let name_idx = self.chunk.add_constant(Constant::String(decl.name.clone()));
                        self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                    }
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

                self.break_targets.push(Vec::new());
                self.continue_targets.push(Vec::new());

                self.compile_statement(body, false)?;

                let continues = self.continue_targets.pop().unwrap();
                for c in continues {
                    self.chunk.patch_jump(c, loop_start);
                }

                self.chunk.emit(OpCode::Jump(loop_start), 0);

                let exit_target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_exit_idx, exit_target);

                let breaks = self.break_targets.pop().unwrap();
                for b in breaks {
                    self.chunk.patch_jump(b, exit_target);
                }
            }
            Statement::For { init, condition, update, body } => {
                self.symbols.enter_scope();
                if let Some(init_stmt) = init {
                    self.compile_statement(init_stmt, false)?;
                }

                let loop_start = self.chunk.instructions.len() as i32;
                let mut jump_exit_idx = None;
                if let Some(cond_expr) = condition {
                    self.compile_expression(cond_expr)?;
                    jump_exit_idx = Some(self.chunk.emit(OpCode::JumpIfFalse(0), 0));
                }

                self.break_targets.push(Vec::new());
                self.continue_targets.push(Vec::new());

                self.compile_statement(body, false)?;

                let update_start = self.chunk.instructions.len() as i32;
                if let Some(up_expr) = update {
                    self.compile_expression(up_expr)?;
                    self.chunk.emit(OpCode::Pop, 0);
                }

                let continues = self.continue_targets.pop().unwrap();
                for c in continues {
                    self.chunk.patch_jump(c, update_start);
                }

                self.chunk.emit(OpCode::Jump(loop_start), 0);

                let exit_target = self.chunk.instructions.len() as i32;
                if let Some(exit_idx) = jump_exit_idx {
                    self.chunk.patch_jump(exit_idx, exit_target);
                }

                let breaks = self.break_targets.pop().unwrap();
                for b in breaks {
                    self.chunk.patch_jump(b, exit_target);
                }

                let popped = self.symbols.exit_scope();
                for _ in 0..popped {
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::Switch { discriminant, cases } => {
                self.compile_expression(discriminant)?;
                let disc_slot = self.symbols.define_local(format!("@switch_{}", self.chunk.instructions.len()));
                self.chunk.emit(OpCode::SetLocal(disc_slot), 0);
                self.chunk.emit(OpCode::Pop, 0);

                self.break_targets.push(Vec::new());

                let mut body_jumps = Vec::new();
                let mut default_idx = None;

                for (idx, case) in cases.iter().enumerate() {
                    if let Some(test_expr) = &case.test {
                        self.chunk.emit(OpCode::GetLocal(disc_slot), 0);
                        self.compile_expression(test_expr)?;
                        self.chunk.emit(OpCode::StrictEq, 0);
                        let jump_not_match = self.chunk.emit(OpCode::JumpIfFalse(0), 0);
                        let jump_body = self.chunk.emit(OpCode::Jump(0), 0);
                        let next_test = self.chunk.instructions.len() as i32;
                        self.chunk.patch_jump(jump_not_match, next_test);
                        body_jumps.push((idx, jump_body));
                    } else {
                        default_idx = Some(idx);
                    }
                }

                let jump_no_case_matched = self.chunk.emit(OpCode::Jump(0), 0);

                let mut body_starts = Vec::new();
                for case in cases {
                    let start = self.chunk.instructions.len() as i32;
                    body_starts.push(start);
                    for s in &case.consequent {
                        self.compile_statement(s, false)?;
                    }
                }

                let exit_target = self.chunk.instructions.len() as i32;

                for (idx, jmp) in body_jumps {
                    let target = body_starts[idx];
                    self.chunk.patch_jump(jmp, target);
                }

                if let Some(def_i) = default_idx {
                    self.chunk.patch_jump(jump_no_case_matched, body_starts[def_i]);
                } else {
                    self.chunk.patch_jump(jump_no_case_matched, exit_target);
                }

                let breaks = self.break_targets.pop().unwrap();
                for b in breaks {
                    self.chunk.patch_jump(b, exit_target);
                }
            }
            Statement::Break => {
                if let Some(targets) = self.break_targets.last_mut() {
                    let jmp = self.chunk.emit(OpCode::Jump(0), 0);
                    targets.push(jmp);
                }
            }
            Statement::Continue => {
                if let Some(targets) = self.continue_targets.last_mut() {
                    let jmp = self.chunk.emit(OpCode::Jump(0), 0);
                    targets.push(jmp);
                }
            }
            Statement::TryCatch { try_block, catch_param, catch_block, finally_block } => {
                let catch_jump_idx = self.chunk.emit(OpCode::PushCatch(0), 0);
                self.compile_statement(try_block, false)?;
                self.chunk.emit(OpCode::PopCatch, 0);

                let jump_over_catch = self.chunk.emit(OpCode::Jump(0), 0);
                let catch_start = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(catch_jump_idx, catch_start);

                if let Some(catch_stmt) = catch_block {
                    self.symbols.enter_scope();
                    if let Some(param) = catch_param {
                        let slot = self.symbols.define_local(param.clone());
                        self.chunk.emit(OpCode::SetLocal(slot), 0);
                    }
                    self.compile_statement(catch_stmt, false)?;
                    let popped = self.symbols.exit_scope();
                    for _ in 0..popped {
                        self.chunk.emit(OpCode::Pop, 0);
                    }
                }

                let exit_target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_over_catch, exit_target);

                if let Some(finally_stmt) = finally_block {
                    self.compile_statement(finally_stmt, false)?;
                }
            }
            Statement::Throw(expr) => {
                self.compile_expression(expr)?;
                self.chunk.emit(OpCode::Throw, 0);
            }
            Statement::Return(opt_expr) => {
                if let Some(expr) = opt_expr {
                    self.compile_expression(expr)?;
                } else {
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }
                self.chunk.emit(OpCode::Return, 0);
            }
            Statement::FunctionDecl { name, params, body } => {
                let def = Self::compile_function(Some(name.clone()), params.clone(), body)?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
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
            Statement::AsyncFunctionDecl { name, params, body } => {
                let def = Self::compile_function(Some(name.clone()), params.clone(), body)?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateAsyncFunction(const_idx), 0);
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
            Statement::GeneratorDecl { name, params, body } => {
                let def = Self::compile_function(Some(name.clone()), params.clone(), body)?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateGenerator(const_idx), 0);
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
            Statement::Import { specifiers, source, .. } => {
                let src_idx = self.chunk.add_constant(Constant::String(source.clone()));
                self.chunk.emit(OpCode::ImportModule(src_idx), 0);
                for (name, alias) in specifiers {
                    let bind_name = alias.as_ref().unwrap_or(name);
                    self.chunk.emit(OpCode::Dup, 0);
                    let prop_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                    if self.symbols.scope_depth > 0 {
                        let slot = self.symbols.define_local(bind_name.clone());
                        self.chunk.emit(OpCode::SetLocal(slot), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                    } else {
                        let name_idx = self.chunk.add_constant(Constant::String(bind_name.clone()));
                        self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                    }
                }
                self.chunk.emit(OpCode::Pop, 0);
            }
            Statement::Export { decl, is_default } => {
                if let Some(stmt) = decl {
                    match &**stmt {
                        Statement::VarDecl { declarations, .. } => {
                            self.compile_statement(stmt, false)?;
                            for decl in declarations {
                                let name_idx = self.chunk.add_constant(Constant::String(decl.name.clone()));
                                self.chunk.emit(OpCode::Export(name_idx), 0);
                            }
                        }
                        Statement::FunctionDecl { name, .. }
                        | Statement::AsyncFunctionDecl { name, .. } => {
                            self.compile_statement(stmt, false)?;
                            let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                            self.chunk.emit(OpCode::Export(name_idx), 0);
                        }
                        Statement::Expression(expr) if *is_default => {
                            self.compile_expression(expr)?;
                            let name_idx = self.chunk.add_constant(Constant::String("default".to_string()));
                            self.chunk.emit(OpCode::Export(name_idx), 0);
                        }
                        _ => {
                            self.compile_statement(stmt, false)?;
                        }
                    }
                }
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
            Expression::This => {
                self.chunk.emit(OpCode::This, 0);
            }
            Expression::Assignment { target, value } => {
                self.compile_expression(value)?;
                self.chunk.emit(OpCode::Dup, 0);
                if let Some(slot) = self.symbols.resolve_local(target) {
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(target.clone()));
                    self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Expression::MemberAssignment { object, property, value } => {
                self.compile_expression(object)?;
                self.compile_expression(value)?;
                let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                self.chunk.emit(OpCode::SetProp(prop_idx), 0);
            }
            Expression::IndexAssignment { object, index, value } => {
                self.compile_expression(object)?;
                self.compile_expression(index)?;
                self.compile_expression(value)?;
                self.chunk.emit(OpCode::SetElem, 0);
            }
            Expression::Object(props) => {
                self.chunk.emit(OpCode::CreateObject, 0);
                for (key, val_expr) in props {
                    self.chunk.emit(OpCode::Dup, 0);
                    self.compile_expression(val_expr)?;
                    let prop_idx = self.chunk.add_constant(Constant::String(key.clone()));
                    self.chunk.emit(OpCode::SetProp(prop_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Expression::Array(elements) => {
                for el in elements {
                    self.compile_expression(el)?;
                }
                self.chunk.emit(OpCode::CreateArray(elements.len() as u16), 0);
            }
            Expression::Index { object, index } => {
                self.compile_expression(object)?;
                self.compile_expression(index)?;
                self.chunk.emit(OpCode::GetElem, 0);
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
                    BinaryOp::Instanceof => self.chunk.emit(OpCode::Instanceof, 0),
                    BinaryOp::In => self.chunk.emit(OpCode::In, 0),
                    BinaryOp::BitwiseAnd => self.chunk.emit(OpCode::BitAnd, 0),
                    BinaryOp::BitwiseOr => self.chunk.emit(OpCode::BitOr, 0),
                    BinaryOp::BitwiseXor => self.chunk.emit(OpCode::BitXor, 0),
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => unreachable!(),
                };
            }
            Expression::Unary { op, expr } => {
                self.compile_expression(expr)?;
                match op {
                    UnaryOp::Pos => self.chunk.emit(OpCode::Pos, 0),
                    UnaryOp::Neg => self.chunk.emit(OpCode::Neg, 0),
                    UnaryOp::Not => self.chunk.emit(OpCode::Not, 0),
                    UnaryOp::BitNot => self.chunk.emit(OpCode::BitNot, 0),
                    UnaryOp::Typeof => self.chunk.emit(OpCode::Typeof, 0),
                    UnaryOp::Void => self.chunk.emit(OpCode::Void, 0),
                    UnaryOp::Delete => self.chunk.emit(OpCode::Delete, 0),
                };
            }
            Expression::Conditional { condition, then_expr, else_expr } => {
                self.compile_expression(condition)?;
                let jump_false_idx = self.chunk.emit(OpCode::JumpIfFalse(0), 0);
                self.compile_expression(then_expr)?;
                let jump_exit_idx = self.chunk.emit(OpCode::Jump(0), 0);
                let else_start = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_false_idx, else_start);
                self.compile_expression(else_expr)?;
                let exit_target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_exit_idx, exit_target);
            }
            Expression::Call { callee, args } => {
                match callee.as_ref() {
                    Expression::Member { object, property } => {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    }
                    Expression::Index { object, index } => {
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        self.compile_expression(index)?;
                        self.chunk.emit(OpCode::GetElem, 0);
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                    }
                    _ => {
                        self.compile_expression(callee)?;
                        for arg in args {
                            self.compile_expression(arg)?;
                        }
                        self.chunk.emit(OpCode::Call(args.len() as u8), 0);
                    }
                }
            }
            Expression::Member { object, property } => {
                self.compile_expression(object)?;
                let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                self.chunk.emit(OpCode::GetProp(prop_idx), 0);
            }
            Expression::FunctionExpr { name, params, body } => {
                let def = Self::compile_function(name.clone(), params.clone(), body)?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
            }
            Expression::New { callee, args } => {
                self.compile_expression(callee)?;
                for arg in args {
                    self.compile_expression(arg)?;
                }
                self.chunk.emit(OpCode::Construct(args.len() as u8), 0);
            }
            Expression::Await(expr) => {
                self.compile_expression(expr)?;
                self.chunk.emit(OpCode::Await, 0);
            }
            Expression::Yield(opt_expr) => {
                if let Some(expr) = opt_expr {
                    self.compile_expression(expr)?;
                } else {
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }
                self.chunk.emit(OpCode::Yield, 0);
            }
            Expression::DynamicImport(expr) => {
                self.compile_expression(expr)?;
                self.chunk.emit(OpCode::DynamicImport, 0);
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

    #[test]
    fn test_compile_object_and_array() {
        let prog = parse("let obj = { a: 1 }; let arr = [10, 20];").unwrap();
        let chunk = Compiler::new().compile(&prog).unwrap();
        assert!(chunk.instructions.contains(&OpCode::CreateObject));
        assert!(chunk.instructions.contains(&OpCode::CreateArray(2)));
    }
}
