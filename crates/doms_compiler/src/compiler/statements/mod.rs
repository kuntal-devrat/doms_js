//! Statement compilation and routing.

use super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;
use std::sync::Arc;

mod control;
mod declarations;
mod exceptions;
mod loops;

impl Compiler {
    pub(crate) fn compile_statement(
        &mut self,
        stmt: &Statement,
        is_last: bool,
    ) -> Result<(), CompileError> {
        match stmt {
            Statement::Expression(expr) => {
                if !self.is_in_function && !self.internal_statements {
                    self.chunk.emit(OpCode::Pop, 0);
                    self.compile_expression(expr)?;
                } else {
                    self.compile_expression(expr)?;
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::VarDecl { kind, declarations } => {
                self.compile_var_decl(kind, declarations)?;
            }
            Statement::Block(stmts) => {
                self.compile_block(stmts, is_last)?;
            }
            Statement::If { condition, then_branch, else_branch } => {
                self.compile_if(condition, then_branch, else_branch, is_last)?;
            }
            Statement::While { condition, body } => {
                self.compile_while(condition, body)?;
            }
            Statement::For { init, condition, update, body } => {
                self.compile_for(init, condition, update, body)?;
            }
            Statement::DoWhile { body, condition } => {
                self.compile_do_while(body, condition)?;
            }
            Statement::ForIn { var_kind, left, right, body } => {
                self.compile_for_in(var_kind, left, right, body)?;
            }
            Statement::ForOf { var_kind, left, right, body, is_await } => {
                self.compile_for_of(var_kind, left, right, body, *is_await)?;
            }
            Statement::Labeled { label, body } => {
                self.compile_labeled(label, body, is_last)?;
            }
            Statement::Switch { discriminant, cases } => {
                self.compile_switch(discriminant, cases)?;
            }
            Statement::Break(opt_label) => {
                self.compile_break(opt_label)?;
            }
            Statement::Continue(opt_label) => {
                self.compile_continue(opt_label)?;
            }
            Statement::TryCatch { try_block, catch_param, catch_block, finally_block } => {
                self.compile_try_catch(try_block, catch_param, catch_block, finally_block)?;
            }
            Statement::Throw(expr) => {
                self.compile_throw(expr)?;
            }
            Statement::Return(opt_expr) => {
                let ret_slot = if let Some(expr) = opt_expr {
                    self.compile_expression(expr)?;
                    let slot = self
                        .symbols
                        .define_local(format!("@ret_{}", self.chunk.instructions.len()));
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                    Some(slot)
                } else {
                    None
                };

                let saved_catch_depth = self.catch_depth;
                let fin_entries = self.finally_stack.clone();
                let saved_finally = self.finally_stack.clone();
                self.finally_stack.clear();
                for entry in fin_entries.iter().rev() {
                    for _ in 0..self.catch_depth.saturating_sub(entry.catch_depth) {
                        self.chunk.emit(OpCode::PopCatch, 0);
                    }
                    self.catch_depth = entry.catch_depth;
                    self.compile_statement(&entry.stmt, false)?;
                }
                self.finally_stack = saved_finally;

                for _ in 0..self.catch_depth {
                    self.chunk.emit(OpCode::PopCatch, 0);
                }
                self.catch_depth = saved_catch_depth;

                if self.break_targets.iter().any(|t| t.iter_slot.is_some()) {
                    for t in self.break_targets.iter().rev() {
                        if let Some(slot) = t.iter_slot {
                            self.chunk.emit(OpCode::PopCatch, 0);
                            self.chunk.emit(OpCode::GetLocal(slot), 0);
                            if t.is_async_iter {
                                self.chunk.emit(OpCode::AsyncIteratorClose(false), 0);
                            } else {
                                self.chunk.emit(OpCode::IteratorClose(false), 0);
                            }
                        }
                    }
                }
                if let Some(slot) = ret_slot {
                    self.chunk.emit(OpCode::GetLocal(slot), 0);
                } else {
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }
                self.chunk.emit(OpCode::Return, 0);
            }
            Statement::FunctionDecl { name, params, body } => {
                let def = Self::compile_function(
                    Some(name.clone()),
                    params.clone(),
                    body,
                    self.is_strict,
                )?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                if (self.is_in_function && self.symbols.scope_depth > 0)
                    || self.symbols.scope_depth > 1
                {
                    let slot =
                        if let Some(local) = self.symbols.get_local_in_current_scope_mut(name) {
                            local.is_declared = true;
                            local.slot
                        } else {
                            self.symbols.define_local(name.clone())
                        };
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::SetGlobalDirect(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::AsyncFunctionDecl { name, params, body } => {
                let def = Self::compile_function(
                    Some(name.clone()),
                    params.clone(),
                    body,
                    self.is_strict,
                )?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateAsyncFunction(const_idx), 0);
                if (self.is_in_function && self.symbols.scope_depth > 0)
                    || self.symbols.scope_depth > 1
                {
                    let slot =
                        if let Some(local) = self.symbols.get_local_in_current_scope_mut(name) {
                            local.is_declared = true;
                            local.slot
                        } else {
                            self.symbols.define_local(name.clone())
                        };
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::SetGlobalDirect(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::GeneratorDecl { name, params, body } => {
                let def = Self::compile_function_full(
                    Some(name.clone()),
                    params.clone(),
                    body,
                    self.is_strict,
                    true,
                    false,
                    false,
                )?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateGenerator(const_idx), 0);
                if (self.is_in_function && self.symbols.scope_depth > 0)
                    || self.symbols.scope_depth > 1
                {
                    let slot =
                        if let Some(local) = self.symbols.get_local_in_current_scope_mut(name) {
                            local.is_declared = true;
                            local.slot
                        } else {
                            self.symbols.define_local(name.clone())
                        };
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::SetGlobalDirect(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::AsyncGeneratorDecl { name, params, body } => {
                let def = Self::compile_function_full(
                    Some(name.clone()),
                    params.clone(),
                    body,
                    self.is_strict,
                    true,
                    false,
                    false,
                )?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateAsyncGenerator(const_idx), 0);
                if (self.is_in_function && self.symbols.scope_depth > 0)
                    || self.symbols.scope_depth > 1
                {
                    let slot =
                        if let Some(local) = self.symbols.get_local_in_current_scope_mut(name) {
                            local.is_declared = true;
                            local.slot
                        } else {
                            self.symbols.define_local(name.clone())
                        };
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::SetGlobalDirect(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::Import { specifiers, source, .. } => {
                self.compile_import(specifiers, source)?;
            }
            Statement::Export { decl, is_default, specifiers, source, is_all, all_as } => {
                self.compile_export(decl, *is_default, specifiers, source, *is_all, all_as)?;
            }
            Statement::ClassDecl(class_def) => {
                self.compile_class_decl(class_def)?;
                self.chunk.emit(OpCode::Pop, 0);
            }
            Statement::With { object, body } => {
                self.compile_expression(object)?;
                self.chunk.emit(OpCode::RequireObjectCoercible, 0);
                self.emit_push_scope();
                self.with_depth += 1;
                self.compile_statement(body, is_last)?;
                self.with_depth -= 1;
                self.emit_pop_scope();
            }
            Statement::Empty | Statement::Debugger => {}
        }
        Ok(())
    }
}
