use super::super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

impl Compiler {
    pub(crate) fn compile_try_catch(
        &mut self,
        try_block: &Statement,
        catch_param: &Option<String>,
        catch_block: &Option<Box<Statement>>,
        finally_block: &Option<Box<Statement>>,
    ) -> Result<(), CompileError> {
        if let Some(finally_stmt) = finally_block {
            let kind_slot = self.symbols.define_local(format!("@finally_kind_{}", self.chunk.instructions.len()));
            let val_slot = self.symbols.define_local(format!("@finally_val_{}", self.chunk.instructions.len()));

            let fin_entry = crate::compiler::FinallyEntry {
                stmt: *finally_stmt.clone(),
                scope_depth: self.block_scope_depth,
                break_depth: self.break_targets.len(),
                catch_depth: self.catch_depth,
            };
            self.finally_stack.push(fin_entry);

            let finally_jump_idx = self.chunk.emit(OpCode::PushFinally(0, 0, kind_slot), 0);
            self.catch_depth += 1;

            if let Some(catch_stmt) = catch_block {
                let catch_jump_idx = self.chunk.emit(OpCode::PushCatch(0), 0);
                self.catch_depth += 1;
                self.compile_statement(try_block, false)?;
                if let Statement::Block(stmts) = try_block {
                    if stmts.is_empty() && !self.is_in_function && !self.internal_statements {
                        self.chunk.emit(OpCode::Pop, 0);
                        self.chunk.emit(OpCode::LoadUndefined, 0);
                    }
                }
                self.chunk.emit(OpCode::PopCatch, 0);
                self.catch_depth -= 1;

                let jump_over_catch = self.chunk.emit(OpCode::Jump(0), 0);
                let catch_start = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(catch_jump_idx, catch_start);

                self.symbols.enter_scope();
                if let Some(param) = catch_param {
                    let slot = self.symbols.define_local(param.clone());
                    self.chunk.emit(OpCode::Dup, 0);
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                    let name_idx = self.chunk.add_constant(Constant::String(param.clone()));
                    self.emit_push_catch_scope(name_idx);
                } else {
                    self.chunk.emit(OpCode::Pop, 0);
                }
                if !self.is_in_function && !self.internal_statements {
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }
                self.compile_statement(catch_stmt, false)?;
                if catch_param.is_some() {
                    self.emit_pop_scope();
                }
                self.symbols.exit_scope();

                let exit_catch_target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_over_catch, exit_catch_target);
            } else {
                self.compile_statement(try_block, false)?;
                if let Statement::Block(stmts) = try_block {
                    if stmts.is_empty() && !self.is_in_function && !self.internal_statements {
                        self.chunk.emit(OpCode::Pop, 0);
                        self.chunk.emit(OpCode::LoadUndefined, 0);
                    }
                }
            }

            self.chunk.emit(OpCode::PopCatch, 0);
            self.catch_depth -= 1;
            self.finally_stack.pop();

            // Normal completion: kind = 0
            let saved_cptn_slot = if !self.is_in_function && !self.internal_statements {
                let slot = self.symbols.define_local(format!("@finally_cptn_{}", self.chunk.instructions.len()));
                self.chunk.emit(OpCode::SetLocal(slot), 0);
                self.chunk.emit(OpCode::Pop, 0);
                self.chunk.emit(OpCode::LoadUndefined, 0);
                Some(slot)
            } else {
                None
            };

            let c0 = self.chunk.add_constant(Constant::Number(0.0));
            self.chunk.emit(OpCode::LoadConst(c0), 0);
            self.chunk.emit(OpCode::SetLocal(kind_slot), 0);
            self.chunk.emit(OpCode::Pop, 0);
            let jump_to_finally = self.chunk.emit(OpCode::Jump(0), 0);

            // Abrupt / exception completion: kind = 1, val = error
            let finally_catch_target = self.chunk.instructions.len() as i32;
            self.chunk.patch_jump(finally_jump_idx, finally_catch_target);
            self.chunk.emit(OpCode::SetLocal(val_slot), 0);
            self.chunk.emit(OpCode::Pop, 0);
            let c1 = self.chunk.add_constant(Constant::Number(1.0));
            self.chunk.emit(OpCode::LoadConst(c1), 0);
            self.chunk.emit(OpCode::SetLocal(kind_slot), 0);
            self.chunk.emit(OpCode::Pop, 0);
            if !self.is_in_function && !self.internal_statements {
                self.chunk.emit(OpCode::LoadUndefined, 0);
            }

            let finally_start = self.chunk.instructions.len() as i32;
            self.chunk.patch_jump(jump_to_finally, finally_start);
            self.chunk.patch_finally_start(finally_jump_idx, finally_start);

            self.compile_statement(finally_stmt, false)?;

            self.chunk.emit(OpCode::EndFinally(kind_slot), 0);

            if let Some(slot) = saved_cptn_slot {
                self.chunk.emit(OpCode::Pop, 0);
                self.chunk.emit(OpCode::GetLocal(slot), 0);
            }
        } else {
            let catch_jump_idx = self.chunk.emit(OpCode::PushCatch(0), 0);
            self.catch_depth += 1;
            self.compile_statement(try_block, false)?;
            if let Statement::Block(stmts) = try_block {
                if stmts.is_empty() && !self.is_in_function && !self.internal_statements {
                    self.chunk.emit(OpCode::Pop, 0);
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }
            }
            self.chunk.emit(OpCode::PopCatch, 0);
            self.catch_depth -= 1;

            let jump_over_catch = self.chunk.emit(OpCode::Jump(0), 0);
            let catch_start = self.chunk.instructions.len() as i32;
            self.chunk.patch_jump(catch_jump_idx, catch_start);

            if let Some(catch_stmt) = catch_block {
                self.symbols.enter_scope();
                if let Some(param) = catch_param {
                    let slot = self.symbols.define_local(param.clone());
                    self.chunk.emit(OpCode::Dup, 0);
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                    let name_idx = self.chunk.add_constant(Constant::String(param.clone()));
                    self.emit_push_catch_scope(name_idx);
                } else {
                    self.chunk.emit(OpCode::Pop, 0);
                }
                if !self.is_in_function && !self.internal_statements {
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }
                self.compile_statement(catch_stmt, false)?;
                if catch_param.is_some() {
                    self.emit_pop_scope();
                }
                self.symbols.exit_scope();
            }

            let exit_target = self.chunk.instructions.len() as i32;
            self.chunk.patch_jump(jump_over_catch, exit_target);
        }
        Ok(())
    }

    pub(crate) fn compile_throw(
        &mut self,
        expr: &Expression,
    ) -> Result<(), CompileError> {
        self.compile_expression(expr)?;
        self.chunk.emit(OpCode::Throw, 0);
        Ok(())
    }
}
