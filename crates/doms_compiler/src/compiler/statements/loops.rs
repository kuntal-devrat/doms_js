use super::super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::{CompileError, TargetScope};
use doms_parser::ast::*;

fn collect_for_lexical_names(var_kind: &Option<VarKind>, left: &str, body: &Statement) -> Vec<String> {
    if !matches!(var_kind, Some(VarKind::Let) | Some(VarKind::Const)) {
        return Vec::new();
    }
    if !left.starts_with('@') {
        return vec![left.to_string()];
    }
    let mut names = Vec::new();
    match body {
        Statement::VarDecl { declarations, .. } => {
            for d in declarations {
                if !d.name.starts_with('@') {
                    names.push(d.name.clone());
                }
            }
        }
        Statement::Block(stmts) => {
            if let Some(Statement::VarDecl { declarations, .. }) = stmts.first() {
                for d in declarations {
                    if !d.name.starts_with('@') {
                        names.push(d.name.clone());
                    }
                }
            }
        }
        _ => {}
    }
    names
}

impl Compiler {
    pub(crate) fn compile_while(
        &mut self,
        condition: &Expression,
        body: &Statement,
    ) -> Result<(), CompileError> {
        if !self.is_in_function {
            self.chunk.emit(OpCode::Pop, 0);
            self.chunk.emit(OpCode::LoadUndefined, 0);
        }
        let loop_start = self.chunk.instructions.len() as i32;
        self.compile_expression(condition)?;
        let jump_exit_idx = self.chunk.emit(OpCode::JumpIfFalse(0), 0);

        let loop_labels = std::mem::take(&mut self.current_labels);
        let bd = self.break_targets.len();
        self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: bd });
        self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: self.break_targets.len() });

        self.compile_statement(body, false)?;

        let continues = self.continue_targets.pop().unwrap();
        for c in continues.jumps {
            self.chunk.patch_jump(c, loop_start);
        }

        self.chunk.emit(OpCode::Jump(loop_start), 0);

        let exit_target = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(jump_exit_idx, exit_target);

        let breaks = self.break_targets.pop().unwrap();
        for b in breaks.jumps {
            self.chunk.patch_jump(b, exit_target);
        }
        Ok(())
    }

    pub(crate) fn compile_for(
        &mut self,
        init: &Option<Box<Statement>>,
        condition: &Option<Expression>,
        update: &Option<Expression>,
        body: &Statement,
    ) -> Result<(), CompileError> {
        if !self.is_in_function {
            self.chunk.emit(OpCode::Pop, 0);
            self.chunk.emit(OpCode::LoadUndefined, 0);
        }
        let mut for_lex_names = Vec::new();
        if !self.is_in_function {
            if let Some(init_stmt) = init {
                if let Statement::VarDecl { kind, declarations } = init_stmt.as_ref() {
                    if *kind != VarKind::Var {
                        for d in declarations {
                            if !for_lex_names.contains(&d.name) {
                                for_lex_names.push(d.name.clone());
                            }
                        }
                    }
                }
            }
        }
        let has_lex_scope = !for_lex_names.is_empty();
        if has_lex_scope {
            self.chunk.emit(OpCode::CreateObject, 0);
            for name in &for_lex_names {
                self.chunk.emit(OpCode::Dup, 0);
                let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::LoadConst(name_idx), 0);
                self.chunk.emit(OpCode::LoadUndefined, 0);
                self.chunk.emit(OpCode::SetElem, 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
            self.emit_push_scope();
        }

        self.symbols.enter_scope();
        if let Some(init_stmt) = init {
            match init_stmt.as_ref() {
                Statement::Expression(expr) => {
                    self.compile_expression(expr)?;
                    self.chunk.emit(OpCode::Pop, 0);
                }
                _ => {
                    self.compile_statement(init_stmt, false)?;
                }
            }
        }

        let loop_start = self.chunk.instructions.len() as i32;
        let mut jump_exit_idx = None;
        if let Some(cond_expr) = condition {
            self.compile_expression(cond_expr)?;
            jump_exit_idx = Some(self.chunk.emit(OpCode::JumpIfFalse(0), 0));
        }

        let loop_labels = std::mem::take(&mut self.current_labels);
        let bd = self.break_targets.len();
        self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: bd });
        self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: self.break_targets.len() });

        self.compile_statement(body, false)?;

        let update_start = self.chunk.instructions.len() as i32;
        if let Some(up_expr) = update {
            self.compile_expression(up_expr)?;
            self.chunk.emit(OpCode::Pop, 0);
        }

        let continues = self.continue_targets.pop().unwrap();
        for c in continues.jumps {
            self.chunk.patch_jump(c, update_start);
        }

        self.chunk.emit(OpCode::Jump(loop_start), 0);

        let exit_target = self.chunk.instructions.len() as i32;
        if let Some(exit_idx) = jump_exit_idx {
            self.chunk.patch_jump(exit_idx, exit_target);
        }

        let breaks = self.break_targets.pop().unwrap();
        for b in breaks.jumps {
            self.chunk.patch_jump(b, exit_target);
        }

        self.symbols.exit_scope();
        if has_lex_scope {
            self.emit_pop_scope();
        }
        Ok(())
    }

    pub(crate) fn compile_do_while(
        &mut self,
        body: &Statement,
        condition: &Expression,
    ) -> Result<(), CompileError> {
        if !self.is_in_function {
            self.chunk.emit(OpCode::Pop, 0);
            self.chunk.emit(OpCode::LoadUndefined, 0);
        }
        let loop_start = self.chunk.instructions.len() as i32;
        let loop_labels = std::mem::take(&mut self.current_labels);
        let bd = self.break_targets.len();
        self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: bd });
        self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: self.break_targets.len() });

        self.compile_statement(body, false)?;

        let cond_start = self.chunk.instructions.len() as i32;
        let continues = self.continue_targets.pop().unwrap();
        for c in continues.jumps {
            self.chunk.patch_jump(c, cond_start);
        }

        self.compile_expression(condition)?;
        self.chunk.emit(OpCode::JumpIfTrue(loop_start), 0);

        let exit_target = self.chunk.instructions.len() as i32;
        let breaks = self.break_targets.pop().unwrap();
        for b in breaks.jumps {
            self.chunk.patch_jump(b, exit_target);
        }
        Ok(())
    }

    pub(crate) fn compile_for_in(
        &mut self,
        var_kind: &Option<VarKind>,
        left: &str,
        right: &Expression,
        body: &Statement,
    ) -> Result<(), CompileError> {
        if !self.is_in_function {
            self.chunk.emit(OpCode::Pop, 0);
            self.chunk.emit(OpCode::LoadUndefined, 0);
        }
        self.symbols.enter_scope();
        let is_lex = matches!(var_kind, Some(VarKind::Let) | Some(VarKind::Const)) && !left.starts_with('@');
        let local_slot = if left.starts_with('@') {
            if self.symbols.resolve_local(left).is_none() {
                self.symbols.define_local(left.to_string());
            }
            self.symbols.resolve_local(left)
        } else if matches!(var_kind, Some(VarKind::Var)) && self.is_in_function {
            if self.symbols.resolve_local(left).is_none() {
                self.symbols.define_local_at_depth(left.to_string(), 1);
            }
            self.symbols.resolve_local(left)
        } else if !is_lex {
            self.symbols.resolve_local(left)
        } else {
            None
        };

        let tdz_names = collect_for_lexical_names(var_kind, left, body);
        let has_tdz = !tdz_names.is_empty();
        if has_tdz {
            self.chunk.emit(OpCode::CreateObject, 0);
            for name in &tdz_names {
                self.chunk.emit(OpCode::Dup, 0);
                let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::LoadConst(name_idx), 0);
                self.chunk.emit(OpCode::LoadUninitialized, 0);
                self.chunk.emit(OpCode::SetElem, 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
            self.emit_push_scope();
        }

        self.compile_expression(right)?;

        if has_tdz {
            self.emit_pop_scope();
        }
        self.chunk.emit(OpCode::GetKeys, 0);
        let keys_slot = self.symbols.define_local(format!("@keys_{}", self.chunk.instructions.len()));
        self.chunk.emit(OpCode::SetLocal(keys_slot), 0);
        self.chunk.emit(OpCode::Pop, 0);

        let zero_idx = self.chunk.add_constant(Constant::Number(0.0));
        self.chunk.emit(OpCode::LoadConst(zero_idx), 0);
        let idx_slot = self.symbols.define_local(format!("@idx_{}", self.chunk.instructions.len()));
        self.chunk.emit(OpCode::SetLocal(idx_slot), 0);
        self.chunk.emit(OpCode::Pop, 0);

        let loop_start = self.chunk.instructions.len() as i32;

        self.chunk.emit(OpCode::GetLocal(idx_slot), 0);
        self.chunk.emit(OpCode::GetLocal(keys_slot), 0);
        let len_idx = self.chunk.add_constant(Constant::String("length".to_string()));
        self.chunk.emit(OpCode::GetProp(len_idx), 0);
        self.chunk.emit(OpCode::Lt, 0);
        let jump_exit_idx = self.chunk.emit(OpCode::JumpIfFalse(0), 0);

        if is_lex {
            self.chunk.emit(OpCode::CreateObject, 0);
            self.chunk.emit(OpCode::Dup, 0);
            let left_idx = self.chunk.add_constant(Constant::String(left.to_string()));
            self.chunk.emit(OpCode::LoadConst(left_idx), 0);
            self.chunk.emit(OpCode::GetLocal(keys_slot), 0);
            self.chunk.emit(OpCode::GetLocal(idx_slot), 0);
            self.chunk.emit(OpCode::GetElem, 0);
            self.chunk.emit(OpCode::SetElem, 0);
            self.chunk.emit(OpCode::Pop, 0);
            self.emit_push_scope();
        } else {
            self.chunk.emit(OpCode::GetLocal(keys_slot), 0);
            self.chunk.emit(OpCode::GetLocal(idx_slot), 0);
            self.chunk.emit(OpCode::GetElem, 0);

            if let Some(slot) = local_slot {
                self.chunk.emit(OpCode::SetLocal(slot), 0);
                self.chunk.emit(OpCode::Pop, 0);
            } else {
                let name_idx = self.chunk.add_constant(Constant::String(left.to_string()));
                self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
        }

        let loop_labels = std::mem::take(&mut self.current_labels);
        let bd = self.break_targets.len();
        self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: bd });
        self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new(), scope_depth: self.block_scope_depth, iter_slot: None, is_async_iter: false, break_depth: self.break_targets.len() });

        self.compile_statement(body, false)?;

        let update_start = self.chunk.instructions.len() as i32;
        if is_lex {
            self.emit_pop_scope();
        }
        self.chunk.emit(OpCode::GetLocal(idx_slot), 0);
        let one_idx = self.chunk.add_constant(Constant::Number(1.0));
        self.chunk.emit(OpCode::LoadConst(one_idx), 0);
        self.chunk.emit(OpCode::Add, 0);
        self.chunk.emit(OpCode::SetLocal(idx_slot), 0);
        self.chunk.emit(OpCode::Pop, 0);

        let continues = self.continue_targets.pop().unwrap();
        for c in continues.jumps {
            self.chunk.patch_jump(c, update_start);
        }

        self.chunk.emit(OpCode::Jump(loop_start), 0);

        let exit_target = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(jump_exit_idx, exit_target);

        let breaks = self.break_targets.pop().unwrap();
        for b in breaks.jumps {
            self.chunk.patch_jump(b, exit_target);
        }

        self.symbols.exit_scope();
        Ok(())
    }

    pub(crate) fn compile_for_of(
        &mut self,
        var_kind: &Option<VarKind>,
        left: &str,
        right: &Expression,
        body: &Statement,
        is_await: bool,
    ) -> Result<(), CompileError> {
        if !self.is_in_function {
            self.chunk.emit(OpCode::Pop, 0);
            self.chunk.emit(OpCode::LoadUndefined, 0);
        }
        self.symbols.enter_scope();
        let is_lex = matches!(var_kind, Some(VarKind::Let) | Some(VarKind::Const)) && !left.starts_with('@');
        let local_slot = if left.starts_with('@') {
            if self.symbols.resolve_local(left).is_none() {
                self.symbols.define_local(left.to_string());
            }
            self.symbols.resolve_local(left)
        } else if matches!(var_kind, Some(VarKind::Var)) && self.is_in_function {
            if self.symbols.resolve_local(left).is_none() {
                self.symbols.define_local_at_depth(left.to_string(), 1);
            }
            self.symbols.resolve_local(left)
        } else if !is_lex {
            self.symbols.resolve_local(left)
        } else {
            None
        };

        let tdz_names = collect_for_lexical_names(var_kind, left, body);
        let has_tdz = !tdz_names.is_empty();
        if has_tdz {
            self.chunk.emit(OpCode::CreateObject, 0);
            for name in &tdz_names {
                self.chunk.emit(OpCode::Dup, 0);
                let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::LoadConst(name_idx), 0);
                self.chunk.emit(OpCode::LoadUninitialized, 0);
                self.chunk.emit(OpCode::SetElem, 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
            self.emit_push_scope();
        }

        self.compile_expression(right)?;

        if has_tdz {
            self.emit_pop_scope();
        }
        if is_await {
            self.chunk.emit(OpCode::GetAsyncIterator, 0);
        } else {
            self.chunk.emit(OpCode::GetIterator, 0);
        }
        let iter_slot = self.symbols.define_local(format!("@iter_{}", self.chunk.instructions.len()));
        self.chunk.emit(OpCode::SetLocal(iter_slot), 0);
        self.chunk.emit(OpCode::Pop, 0);

        let loop_labels = std::mem::take(&mut self.current_labels);
        let bd = self.break_targets.len();
        self.break_targets.push(TargetScope {
            labels: loop_labels.clone(),
            jumps: Vec::new(),
            scope_depth: self.block_scope_depth,
            iter_slot: Some(iter_slot),
            is_async_iter: is_await,
            break_depth: bd,
        });
        self.continue_targets.push(TargetScope {
            labels: loop_labels,
            jumps: Vec::new(),
            scope_depth: self.block_scope_depth,
            iter_slot: None,
            is_async_iter: false,
            break_depth: self.break_targets.len(),
        });

        let loop_start = self.chunk.instructions.len() as i32;
        self.chunk.emit(OpCode::GetLocal(iter_slot), 0);
        let jump_exit_idx = if is_await {
            self.chunk.emit(OpCode::AsyncIteratorNext(0), 0)
        } else {
            self.chunk.emit(OpCode::IteratorNext(0), 0)
        };

        let catch_jump_idx = self.chunk.emit(OpCode::PushCatch(0), 0);

        if is_lex {
            let val_slot = self.symbols.define_local(format!("@val_{}", self.chunk.instructions.len()));
            self.chunk.emit(OpCode::SetLocal(val_slot), 0);
            self.chunk.emit(OpCode::Pop, 0);

            self.chunk.emit(OpCode::CreateObject, 0);
            self.chunk.emit(OpCode::Dup, 0);
            let left_idx = self.chunk.add_constant(Constant::String(left.to_string()));
            self.chunk.emit(OpCode::LoadConst(left_idx), 0);
            self.chunk.emit(OpCode::GetLocal(val_slot), 0);
            self.chunk.emit(OpCode::SetElem, 0);
            self.chunk.emit(OpCode::Pop, 0);
            self.emit_push_scope();
        } else {
            if let Some(slot) = local_slot {
                self.chunk.emit(OpCode::SetLocal(slot), 0);
                self.chunk.emit(OpCode::Pop, 0);
            } else {
                let name_idx = self.chunk.add_constant(Constant::String(left.to_string()));
                self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
        }

        self.compile_statement(body, false)?;

        let update_start = self.chunk.instructions.len() as i32;
        if is_lex {
            self.emit_pop_scope();
        }
        self.chunk.emit(OpCode::PopCatch, 0);

        let continues = self.continue_targets.pop().unwrap();
        for c in continues.jumps {
            self.chunk.patch_jump(c, update_start);
        }

        self.chunk.emit(OpCode::Jump(loop_start), 0);

        let catch_start = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(catch_jump_idx, catch_start);
        let err_slot = self.symbols.define_local(format!("@err_{}", self.chunk.instructions.len()));
        self.chunk.emit(OpCode::SetLocal(err_slot), 0);
        self.chunk.emit(OpCode::Pop, 0);
        self.chunk.emit(OpCode::GetLocal(iter_slot), 0);
        if is_await {
            self.chunk.emit(OpCode::AsyncIteratorClose(true), 0);
        } else {
            self.chunk.emit(OpCode::IteratorClose(true), 0);
        }
        self.chunk.emit(OpCode::GetLocal(err_slot), 0);
        self.chunk.emit(OpCode::Throw, 0);

        let final_exit = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(jump_exit_idx, final_exit);

        let breaks = self.break_targets.pop().unwrap();
        for b in breaks.jumps {
            self.chunk.patch_jump(b, final_exit);
        }

        self.symbols.exit_scope();
        Ok(())
    }
}
