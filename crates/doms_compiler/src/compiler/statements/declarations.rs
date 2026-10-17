use super::super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

impl Compiler {
    pub(crate) fn compile_var_decl(
        &mut self,
        kind: &VarKind,
        declarations: &[VarDeclarator],
    ) -> Result<(), CompileError> {
        for decl in declarations {
            if decl.name.starts_with("@touch_") || decl.name.starts_with("__touch_") {
                if let Some(Expression::Identifier(ref target)) = decl.init {
                    self.touched_bindings.insert(target.clone());
                }
            }
        }
        for decl in declarations {
            if let Some(init_expr) = &decl.init {
                match init_expr {
                    Expression::Conditional { condition, then_expr, else_expr } if !decl.name.starts_with('@') => {
                        self.compile_conditional_with_name(condition, then_expr, else_expr, Some(&decl.name))?;
                    }
                    Expression::ClassExpr(class_def) if !decl.name.starts_with('@') && class_def.name.is_none() => {
                        self.compile_class_definition_with_name(class_def, Some(decl.name.clone()))?;
                    }
                    _ => {
                        self.compile_expression(init_expr)?;
                        if !decl.name.starts_with('@') && Self::is_anonymous_function_definition(init_expr) {
                            let name_idx = self.chunk.add_constant(Constant::String(decl.name.clone()));
                            self.chunk.emit(OpCode::SetFunctionName(name_idx), 0);
                        }
                    }
                }
                if decl.name.starts_with("__destruct_") || decl.name.starts_with("@destruct_") || decl.name.starts_with("@dstr_") {
                    self.chunk.emit(OpCode::RequireObjectCoercible, 0);
                }

                if decl.name.starts_with('@') {
                    let slot = if let Some(existing) = self.symbols.resolve_local(&decl.name) {
                        existing
                    } else {
                        self.symbols.define_local(decl.name.clone())
                    };
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else if *kind == VarKind::Var && !self.is_in_function {
                    let is_touched = self.touched_bindings.remove(&decl.name);
                    let name_idx = self.chunk.add_constant(Constant::String(decl.name.clone()));
                    if is_touched {
                        self.chunk.emit(OpCode::SetGlobalDirect(name_idx), 0);
                    } else {
                        self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    }
                    self.chunk.emit(OpCode::Pop, 0);
                } else if self.is_in_function && self.symbols.scope_depth > 0 {
                    let slot = if *kind == VarKind::Var {
                        if let Some(existing_slot) = self.symbols.resolve_local(&decl.name) {
                            existing_slot
                        } else {
                            self.symbols.define_local_at_depth(decl.name.clone(), 1)
                        }
                    } else {
                        if self.symbols.is_local_in_current_scope(&decl.name) {
                            return Err(CompileError::DuplicateDeclaration(decl.name.clone()));
                        }
                        self.symbols.define_local(decl.name.clone())
                    };
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    if !self.is_in_function && self.symbols.scope_depth > 0 {
                        if self.symbols.is_local_in_current_scope(&decl.name) {
                            return Err(CompileError::DuplicateDeclaration(decl.name.clone()));
                        }
                        self.symbols.define_local(decl.name.clone());
                    }
                    let is_touched = self.touched_bindings.remove(&decl.name);
                    let name_idx = self.chunk.add_constant(Constant::String(decl.name.clone()));
                    if *kind == VarKind::Const && !self.is_eval && self.symbols.scope_depth == 0 {
                        self.chunk.emit(OpCode::InitConst(name_idx), 0);
                    } else if is_touched {
                        self.chunk.emit(OpCode::SetGlobalDirect(name_idx), 0);
                    } else {
                        self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    }
                    self.chunk.emit(OpCode::Pop, 0);
                }
            } else if decl.name.starts_with('@') {
                self.chunk.emit(OpCode::LoadUndefined, 0);
                let slot = if let Some(existing) = self.symbols.resolve_local(&decl.name) {
                    existing
                } else {
                    self.symbols.define_local(decl.name.clone())
                };
                self.chunk.emit(OpCode::SetLocal(slot), 0);
                self.chunk.emit(OpCode::Pop, 0);
            } else if *kind == VarKind::Let {
                self.chunk.emit(OpCode::LoadUndefined, 0);
                if self.is_in_function && self.symbols.scope_depth > 0 {
                    if self.symbols.is_local_in_current_scope(&decl.name) {
                        return Err(CompileError::DuplicateDeclaration(decl.name.clone()));
                    }
                    let slot = self.symbols.define_local(decl.name.clone());
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    if !self.is_in_function && self.symbols.scope_depth > 0 {
                        if self.symbols.is_local_in_current_scope(&decl.name) {
                            return Err(CompileError::DuplicateDeclaration(decl.name.clone()));
                        }
                        self.symbols.define_local(decl.name.clone());
                    }
                    let name_idx = self.chunk.add_constant(Constant::String(decl.name.clone()));
                    self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            } else if *kind == VarKind::Var && self.is_in_function {
                if self.symbols.resolve_local(&decl.name).is_none() {
                    self.symbols.define_local_at_depth(decl.name.clone(), 1);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn compile_block(
        &mut self,
        stmts: &[Statement],
        is_last: bool,
    ) -> Result<(), CompileError> {
        let is_top_level_block = !self.is_in_function;
        let mut block_lex_names = Vec::new();
        if is_top_level_block {
            for s in stmts {
                match s {
                    Statement::VarDecl { kind, declarations } if *kind != VarKind::Var => {
                        for d in declarations {
                            if !d.name.starts_with('@') && !block_lex_names.contains(&d.name) {
                                block_lex_names.push(d.name.clone());
                            }
                        }
                    }
                    Statement::ClassDecl(c) => {
                        if let Some(ref n) = c.name {
                            if !block_lex_names.contains(n) {
                                block_lex_names.push(n.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let has_lex_scope = !block_lex_names.is_empty();
        if has_lex_scope {
            self.chunk.emit(OpCode::CreateObject, 0);
            for name in &block_lex_names {
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
        let len = stmts.len();
        for (idx, s) in stmts.iter().enumerate() {
            self.compile_statement(s, is_last && idx == len - 1)?;
        }
        self.symbols.exit_scope();

        if has_lex_scope {
            self.emit_pop_scope();
        }
        Ok(())
    }
}
