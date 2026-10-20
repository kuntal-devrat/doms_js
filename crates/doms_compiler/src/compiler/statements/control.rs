use super::super::Compiler;
use crate::bytecode::{Constant, OpCode};
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
        self.compile_expression(discriminant)?;
        let disc_slot = self.symbols.define_local(format!("@switch_{}", self.chunk.instructions.len()));
        self.chunk.emit(OpCode::SetLocal(disc_slot), 0);
        self.chunk.emit(OpCode::Pop, 0);

        self.symbols.enter_scope();

        let mut case_lex_names = Vec::new();
        for case in cases {
            for s in &case.consequent {
                match s {
                    Statement::VarDecl { kind, declarations } if *kind != VarKind::Var => {
                        for d in declarations {
                            if !case_lex_names.contains(&d.name) {
                                case_lex_names.push(d.name.clone());
                            }
                        }
                    }
                    Statement::ClassDecl(c) => {
                        if let Some(ref n) = c.name {
                            if !case_lex_names.contains(n) {
                                case_lex_names.push(n.clone());
                            }
                        }
                    }
                    Statement::FunctionDecl { name, .. }
                    | Statement::AsyncFunctionDecl { name, .. }
                    | Statement::GeneratorDecl { name, .. }
                    | Statement::AsyncGeneratorDecl { name, .. } => {
                        if !name.is_empty() && !case_lex_names.contains(name) {
                            case_lex_names.push(name.clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        for name in &case_lex_names {
            self.symbols.define_pre_declared(name.clone());
        }

        if !self.is_in_function && !self.internal_statements {
            self.chunk.emit(OpCode::Pop, 0);
            self.chunk.emit(OpCode::LoadUndefined, 0);
        }

        self.break_targets.push(TargetScope { labels: Vec::new(), jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: self.break_targets.len(), catch_depth: self.catch_depth });

        let has_lex_scope = !case_lex_names.is_empty();
        if has_lex_scope {
            self.chunk.emit(OpCode::CreateObject, 0);
            for name in &case_lex_names {
                self.chunk.emit(OpCode::Dup, 0);
                let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::LoadConst(name_idx), 0);
                self.chunk.emit(OpCode::LoadUndefined, 0);
                self.chunk.emit(OpCode::SetElem, 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
            self.emit_push_scope();
        }

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
        if has_lex_scope {
            self.emit_pop_scope();
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
            self.break_targets.push(TargetScope { labels, jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: bd, catch_depth: self.catch_depth });
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
        let (target_idx, target_scope, target_break_depth, target_catch_depth) = if let Some(label) = opt_label {
            if let Some(idx) = self.break_targets.iter().rposition(|t| t.labels.iter().any(|l| l == label)) {
                let t = &self.break_targets[idx];
                (idx, t.scope_depth, t.break_depth, t.catch_depth)
            } else {
                return Err(CompileError::General(format!("Undefined label '{}'", label)));
            }
        } else if let Some(idx) = self.break_targets.len().checked_sub(1) {
            let t = &self.break_targets[idx];
            (idx, t.scope_depth, t.break_depth, t.catch_depth)
        } else {
            return Err(CompileError::General("Illegal break statement".into()));
        };

        let saved_catch_depth = self.catch_depth;
        let saved_scope_depth = self.block_scope_depth;

        let fin_entries: Vec<crate::compiler::FinallyEntry> = self.finally_stack.iter()
            .filter(|e| e.break_depth >= target_break_depth)
            .cloned()
            .collect();
        let saved_finally = self.finally_stack.clone();
        self.finally_stack.retain(|e| e.break_depth < target_break_depth);
        for entry in fin_entries.into_iter().rev() {
            for _ in 0..self.catch_depth.saturating_sub(entry.catch_depth) {
                self.chunk.emit(OpCode::PopCatch, 0);
            }
            self.catch_depth = entry.catch_depth;
            self.compile_statement(&entry.stmt, false)?;
        }
        self.finally_stack = saved_finally;

        for t in self.break_targets[target_break_depth..].iter().rev() {
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
        for _ in 0..self.catch_depth.saturating_sub(target_catch_depth) {
            self.chunk.emit(OpCode::PopCatch, 0);
        }

        for _ in 0..self.block_scope_depth.saturating_sub(target_scope) {
            self.chunk.emit(OpCode::PopScope, 0);
        }

        let jmp = self.chunk.emit(OpCode::Jump(0), 0);
        self.break_targets[target_idx].jumps.push(jmp);
        self.catch_depth = saved_catch_depth;
        self.block_scope_depth = saved_scope_depth;
        Ok(())
    }

    pub(crate) fn compile_continue(
        &mut self,
        opt_label: &Option<String>,
    ) -> Result<(), CompileError> {
        let (target_idx, target_scope, target_break_depth, target_catch_depth) = if let Some(label) = opt_label {
            if let Some(idx) = self.continue_targets.iter().rposition(|t| t.labels.iter().any(|l| l == label)) {
                let t = &self.continue_targets[idx];
                (idx, t.scope_depth, t.break_depth, t.catch_depth)
            } else {
                return Err(CompileError::General(format!("Undefined label '{}'", label)));
            }
        } else if let Some(idx) = self.continue_targets.len().checked_sub(1) {
            let t = &self.continue_targets[idx];
            (idx, t.scope_depth, t.break_depth, t.catch_depth)
        } else {
            return Err(CompileError::General("Illegal continue statement".into()));
        };

        let saved_catch_depth = self.catch_depth;
        let saved_scope_depth = self.block_scope_depth;

        let fin_entries: Vec<crate::compiler::FinallyEntry> = self.finally_stack.iter()
            .filter(|e| e.break_depth >= target_break_depth)
            .cloned()
            .collect();
        let saved_finally = self.finally_stack.clone();
        self.finally_stack.retain(|e| e.break_depth < target_break_depth);
        for entry in fin_entries.into_iter().rev() {
            for _ in 0..self.catch_depth.saturating_sub(entry.catch_depth) {
                self.chunk.emit(OpCode::PopCatch, 0);
            }
            self.catch_depth = entry.catch_depth;
            self.compile_statement(&entry.stmt, false)?;
        }
        self.finally_stack = saved_finally;

        for t in self.break_targets[target_break_depth..].iter().rev() {
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
        for _ in 0..self.catch_depth.saturating_sub(target_catch_depth) {
            self.chunk.emit(OpCode::PopCatch, 0);
        }

        for _ in 0..self.block_scope_depth.saturating_sub(target_scope) {
            self.chunk.emit(OpCode::PopScope, 0);
        }

        let jmp = self.chunk.emit(OpCode::Jump(0), 0);
        self.continue_targets[target_idx].jumps.push(jmp);
        self.catch_depth = saved_catch_depth;
        self.block_scope_depth = saved_scope_depth;
        Ok(())
    }
}
