use super::super::Compiler;
use crate::bytecode::OpCode;
use crate::compiler::{CompileError, TargetScope};
use doms_parser::ast::*;

impl Compiler {
    pub(crate) fn compile_if(
        &mut self,
        condition: &Expression,
        then_branch: &Statement,
        else_branch: &Option<Box<Statement>>,
        is_last: bool,
    ) -> Result<(), CompileError> {
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
        Ok(())
    }

    pub(crate) fn compile_switch(
        &mut self,
        discriminant: &Expression,
        cases: &[SwitchCase],
    ) -> Result<(), CompileError> {
        self.symbols.enter_scope();
        self.compile_expression(discriminant)?;
        let disc_slot = self.symbols.define_local(format!("@switch_{}", self.chunk.instructions.len()));
        self.chunk.emit(OpCode::SetLocal(disc_slot), 0);
        self.chunk.emit(OpCode::Pop, 0);

        if !self.is_in_function && !self.internal_statements && !cases.is_empty() {
            self.chunk.emit(OpCode::Pop, 0);
            self.chunk.emit(OpCode::LoadUndefined, 0);
        }

        self.break_targets.push(TargetScope { labels: Vec::new(), jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: self.break_targets.len() });

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
        for b in breaks.jumps {
            self.chunk.patch_jump(b, exit_target);
        }
        self.symbols.exit_scope();
        Ok(())
    }

    pub(crate) fn compile_labeled(
        &mut self,
        label: &str,
        body: &Statement,
        is_last: bool,
    ) -> Result<(), CompileError> {
        self.current_labels.push(label.to_string());
        let is_loop = matches!(body,
            Statement::While { .. } |
            Statement::DoWhile { .. } |
            Statement::For { .. } |
            Statement::ForIn { .. } |
            Statement::ForOf { .. } |
            Statement::Labeled { .. }
        );
        if is_loop {
            self.compile_statement(body, is_last)?;
        } else {
            let labels = std::mem::take(&mut self.current_labels);
            let bd = self.break_targets.len();
            self.break_targets.push(TargetScope { labels, jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: bd });
            self.compile_statement(body, is_last)?;
            let exit_target = self.chunk.instructions.len() as i32;
            let breaks = self.break_targets.pop().unwrap();
            for b in breaks.jumps {
                self.chunk.patch_jump(b, exit_target);
            }
        }
        Ok(())
    }

    pub(crate) fn compile_break(
        &mut self,
        opt_label: &Option<String>,
    ) -> Result<(), CompileError> {
        if let Some(label) = opt_label {
            if let Some(target_idx) = self.break_targets.iter().rposition(|t| t.labels.iter().any(|l| l == label)) {
                let target_scope = self.break_targets[target_idx].scope_depth;
                for t in self.break_targets[target_idx..].iter().rev() {
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
                for _ in 0..self.block_scope_depth.saturating_sub(target_scope) {
                    self.chunk.emit(OpCode::PopScope, 0);
                }
                let jmp = self.chunk.emit(OpCode::Jump(0), 0);
                self.break_targets[target_idx].jumps.push(jmp);
            } else {
                return Err(CompileError::General(format!("Undefined label '{}'", label)));
            }
        } else if let Some(target) = self.break_targets.last_mut() {
            if let Some(slot) = target.iter_slot {
                self.chunk.emit(OpCode::PopCatch, 0);
                self.chunk.emit(OpCode::GetLocal(slot), 0);
                if target.is_async_iter {
                    self.chunk.emit(OpCode::AsyncIteratorClose(false), 0);
                } else {
                    self.chunk.emit(OpCode::IteratorClose(false), 0);
                }
            }
            let target_scope = target.scope_depth;
            for _ in 0..self.block_scope_depth.saturating_sub(target_scope) {
                self.chunk.emit(OpCode::PopScope, 0);
            }
            let jmp = self.chunk.emit(OpCode::Jump(0), 0);
            target.jumps.push(jmp);
        } else {
            return Err(CompileError::General("Illegal break statement".into()));
        }
        Ok(())
    }

    pub(crate) fn compile_continue(
        &mut self,
        opt_label: &Option<String>,
    ) -> Result<(), CompileError> {
        if let Some(label) = opt_label {
            if let Some(target_idx) = self.continue_targets.iter().rposition(|t| t.labels.iter().any(|l| l == label)) {
                let target_scope = self.continue_targets[target_idx].scope_depth;
                let break_depth = self.continue_targets[target_idx].break_depth;
                for t in self.break_targets[break_depth..].iter().rev() {
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
                for _ in 0..self.block_scope_depth.saturating_sub(target_scope) {
                    self.chunk.emit(OpCode::PopScope, 0);
                }
                let jmp = self.chunk.emit(OpCode::Jump(0), 0);
                self.continue_targets[target_idx].jumps.push(jmp);
            } else {
                return Err(CompileError::General(format!("Undefined label '{}'", label)));
            }
        } else if let Some(target) = self.continue_targets.last_mut() {
            let target_scope = target.scope_depth;
            for _ in 0..self.block_scope_depth.saturating_sub(target_scope) {
                self.chunk.emit(OpCode::PopScope, 0);
            }
            let jmp = self.chunk.emit(OpCode::Jump(0), 0);
            target.jumps.push(jmp);
        } else {
            return Err(CompileError::General("Illegal continue statement".into()));
        }
        Ok(())
    }
}
