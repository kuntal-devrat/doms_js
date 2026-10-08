//! AST to bytecode compiler.

use crate::bytecode::{BytecodeChunk, Constant, FunctionDef, OpCode};
use crate::scope::SymbolTable;
use doms_parser::ast::*;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Identifier '{0}' has already been declared")]
    DuplicateDeclaration(String),
    #[error("Variable '{0}' already declared in this scope")]
    DuplicateVariable(String),
    #[error("Too many constants in chunk")]
    TooManyConstants,
    #[error("Too many locals")]
    TooManyLocals,
    #[error("{0}")]
    General(String),
}

#[derive(Debug, Clone)]
pub struct TargetScope {
    pub labels: Vec<String>,
    pub jumps: Vec<usize>,
}

pub struct Compiler {
    pub chunk: BytecodeChunk,
    pub symbols: SymbolTable,
    pub break_targets: Vec<TargetScope>,
    pub continue_targets: Vec<TargetScope>,
    pub current_labels: Vec<String>,
    pub is_in_function: bool,
    pub with_depth: usize,
    pub is_strict: bool,
}

fn collect_var_names(stmts: &[Statement], names: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            Statement::VarDecl { kind: VarKind::Var, declarations } => {
                for decl in declarations {
                    if !names.contains(&decl.name) {
                        names.push(decl.name.clone());
                    }
                }
            }
            Statement::Block(inner) => collect_var_names(inner, names),
            Statement::If { then_branch, else_branch, .. } => {
                collect_var_names(std::slice::from_ref(then_branch), names);
                if let Some(alt) = else_branch {
                    collect_var_names(std::slice::from_ref(alt), names);
                }
            }
            Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                collect_var_names(std::slice::from_ref(body), names);
            }
            Statement::For { init, body, .. } => {
                if let Some(init_stmt) = init {
                    collect_var_names(std::slice::from_ref(init_stmt), names);
                }
                collect_var_names(std::slice::from_ref(body), names);
            }
            Statement::ForIn { body, .. } | Statement::ForOf { body, .. } => {
                collect_var_names(std::slice::from_ref(body), names);
            }
            Statement::With { body, .. } => {
                collect_var_names(std::slice::from_ref(body), names);
            }
            Statement::TryCatch { try_block, catch_block, finally_block, .. } => {
                collect_var_names(std::slice::from_ref(try_block), names);
                if let Some(cb) = catch_block {
                    collect_var_names(std::slice::from_ref(cb), names);
                }
                if let Some(fb) = finally_block {
                    collect_var_names(std::slice::from_ref(fb), names);
                }
            }
            Statement::Labeled { body, .. } => {
                collect_var_names(std::slice::from_ref(body), names);
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    collect_var_names(&case.consequent, names);
                }
            }
            _ => {}
        }
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            chunk: BytecodeChunk::new(),
            symbols: SymbolTable::new(),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
            current_labels: Vec::new(),
            is_in_function: false,
            with_depth: 0,
            is_strict: false,
        }
    }

    pub fn compile(mut self, program: &Program) -> Result<BytecodeChunk, CompileError> {
        let has_directive = program.body.iter()
            .take_while(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { .. }))))
            .any(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { value, has_escape: false })) if value == "use strict"));
        if has_directive {
            self.is_strict = true;
        }

        // Collect top-level declarations for script GlobalDeclarationInstantiation
        let mut top_vars = Vec::new();
        collect_var_names(&program.body, &mut top_vars);

        let mut top_lex_names = Vec::new();
        let mut top_func_names = Vec::new();

        for stmt in &program.body {
            match stmt {
                Statement::VarDecl { kind, declarations } => {
                    if *kind != VarKind::Var {
                        for decl in declarations {
                            top_lex_names.push(decl.name.clone());
                        }
                    }
                }
                Statement::ClassDecl(class_def) => {
                    if let Some(ref n) = class_def.name {
                        top_lex_names.push(n.clone());
                    }
                }
                Statement::FunctionDecl { name, .. }
                | Statement::AsyncFunctionDecl { name, .. } => {
                    top_func_names.push(name.clone());
                }
                _ => {}
            }
        }

        // Early errors: intra-script lexical redeclaration or lexical collision with var/func
        let mut seen_lex = std::collections::HashSet::new();
        for name in &top_lex_names {
            if !seen_lex.insert(name) {
                return Err(CompileError::DuplicateDeclaration(name.clone()));
            }
        }
        for name in &top_lex_names {
            if top_vars.contains(name) || top_func_names.contains(name) {
                return Err(CompileError::DuplicateDeclaration(name.clone()));
            }
        }

        self.chunk.top_lex_names = top_lex_names;
        self.chunk.top_var_names = top_vars;
        self.chunk.top_func_names = top_func_names;

        // Hoist top-level function declarations
        for stmt in &program.body {
            if let Statement::FunctionDecl { name, params, body } = stmt {
                let def = Self::compile_function(Some(name.clone()), params.clone(), body, self.is_strict)?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
        }

        self.chunk.emit(OpCode::LoadUndefined, 0);
        for stmt in &program.body {
            if matches!(stmt, Statement::FunctionDecl { .. }) {
                continue;
            }
            self.compile_statement(stmt, false)?;
        }
        self.chunk.emit(OpCode::Halt, 0);
        Ok(self.chunk)
    }

    pub fn compile_function(
        name: Option<String>,
        params: Vec<String>,
        body: &[Statement],
        outer_is_strict: bool,
    ) -> Result<FunctionDef, CompileError> {
        let has_directive = body.iter()
            .take_while(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { .. }))))
            .any(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { value, has_escape: false })) if value == "use strict"));
        let is_strict = outer_is_strict || has_directive;

        let mut compiler = Compiler::new();
        compiler.is_strict = is_strict;
        compiler.is_in_function = true;
        compiler.symbols.enter_scope();
        for p in &params {
            let local_name = if let Some(rest_name) = p.strip_prefix("...") {
                rest_name
            } else {
                p.as_str()
            };
            compiler.symbols.define_local(local_name.to_string());
        }

        // Hoist var declarations to the function scope
        let mut func_vars = Vec::new();
        collect_var_names(body, &mut func_vars);
        for v in func_vars {
            if compiler.symbols.resolve_local(&v).is_none() {
                compiler.symbols.define_local_at_depth(v, 1);
            }
        }

        // Hoist function declarations to the top of the function
        for stmt in body {
            if let Statement::FunctionDecl { name, params, body } = stmt {
                let def = Self::compile_function(Some(name.clone()), params.clone(), body, compiler.is_strict)?;
                let const_idx = compiler.chunk.add_constant(Constant::Function(Arc::new(def)));
                compiler.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                let slot = compiler.symbols.define_local(name.clone());
                compiler.chunk.emit(OpCode::SetLocal(slot), 0);
                compiler.chunk.emit(OpCode::Pop, 0);
            }
        }

        let total = body.len();
        for (i, stmt) in body.iter().enumerate() {
            if matches!(stmt, Statement::FunctionDecl { .. }) {
                continue;
            }
            let is_last = i == total - 1;
            compiler.compile_statement(stmt, is_last)?;
        }
        // Default return undefined
        compiler.chunk.emit(OpCode::LoadUndefined, 0);
        compiler.chunk.emit(OpCode::Return, 0);
        let local_names: Vec<String> = compiler.symbols.locals.iter().map(|l| l.name.clone()).collect();
        Ok(FunctionDef { name, params, local_names, chunk: compiler.chunk, is_strict })
    }

    fn compile_class_definition(&mut self, class_def: &ClassDefinition) -> Result<(), CompileError> {
        let def = Self::compile_function(class_def.name.clone(), class_def.ctor_params.clone(), &class_def.ctor_body, self.is_strict)?;
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateFunction(const_idx), 0);

        if let Some(ref sup) = class_def.super_class {
            self.compile_expression(sup)?;
            self.chunk.emit(OpCode::InheritClass, 0);
        }

        for element in &class_def.elements {
            self.chunk.emit(OpCode::Dup, 0);
            match &element.key {
                PropertyKey::Literal(s) => {
                    let const_idx = self.chunk.add_constant(Constant::String(s.clone()));
                    self.chunk.emit(OpCode::LoadConst(const_idx), 0);
                }
                PropertyKey::Computed(k_expr) => {
                    self.compile_expression(k_expr)?;
                }
            }
            let kind_flag = match &element.kind {
                ClassElementKind::Method(m_expr) => {
                    self.compile_expression(m_expr)?;
                    0u8
                }
                ClassElementKind::Getter(g_expr) => {
                    self.compile_expression(g_expr)?;
                    2u8
                }
                ClassElementKind::Setter(s_expr) => {
                    self.compile_expression(s_expr)?;
                    4u8
                }
            };
            let flags = if element.is_static { kind_flag | 1 } else { kind_flag };
            self.chunk.emit(OpCode::DefineClassElement(flags), 0);
        }
        Ok(())
    }

    fn compile_statement(&mut self, stmt: &Statement, is_last: bool) -> Result<(), CompileError> {
        match stmt {
            Statement::Expression(expr) => {
                if !self.is_in_function {
                    self.chunk.emit(OpCode::Pop, 0);
                    self.compile_expression(expr)?;
                } else {
                    self.compile_expression(expr)?;
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::VarDecl { kind, declarations } => {
                for decl in declarations {
                    if let Some(init_expr) = &decl.init {
                        self.compile_expression(init_expr)?;

                        if *kind == VarKind::Var && !self.is_in_function {
                            let name_idx = self.chunk.add_constant(Constant::String(decl.name.clone()));
                            self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        } else if self.symbols.scope_depth > 0 {
                            let slot = if let Some(existing_slot) = self.symbols.resolve_local(&decl.name) {
                                existing_slot
                            } else if *kind == VarKind::Var && self.is_in_function {
                                self.symbols.define_local_at_depth(decl.name.clone(), 1)
                            } else {
                                self.symbols.define_local(decl.name.clone())
                            };
                            self.chunk.emit(OpCode::SetLocal(slot), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        } else {
                            let name_idx = self.chunk.add_constant(Constant::String(decl.name.clone()));
                            if *kind == VarKind::Const {
                                self.chunk.emit(OpCode::InitConst(name_idx), 0);
                            } else {
                                self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                            }
                            self.chunk.emit(OpCode::Pop, 0);
                        }
                    } else if *kind == VarKind::Let {
                        self.chunk.emit(OpCode::LoadUndefined, 0);
                        if self.symbols.scope_depth > 0 {
                            let slot = self.symbols.define_local(decl.name.clone());
                            self.chunk.emit(OpCode::SetLocal(slot), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        } else {
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
            }
            Statement::Block(stmts) => {
                self.symbols.enter_scope();
                let len = stmts.len();
                for (idx, s) in stmts.iter().enumerate() {
                    self.compile_statement(s, is_last && idx == len - 1)?;
                }
                self.symbols.exit_scope();
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

                let loop_labels = std::mem::take(&mut self.current_labels);
                self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new() });
                self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new() });

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

                let loop_labels = std::mem::take(&mut self.current_labels);
                self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new() });
                self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new() });

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
            }
            Statement::DoWhile { body, condition } => {
                let loop_start = self.chunk.instructions.len() as i32;
                let loop_labels = std::mem::take(&mut self.current_labels);
                self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new() });
                self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new() });

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
            }
            Statement::ForIn { left, right, body } => {
                self.symbols.enter_scope();
                self.compile_expression(right)?;
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

                self.chunk.emit(OpCode::GetLocal(keys_slot), 0);
                self.chunk.emit(OpCode::GetLocal(idx_slot), 0);
                self.chunk.emit(OpCode::GetElem, 0);

                if let Some(slot) = self.symbols.resolve_local(left) {
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(left.clone()));
                    self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }

                let loop_labels = std::mem::take(&mut self.current_labels);
                self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new() });
                self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new() });

                self.compile_statement(body, false)?;

                let update_start = self.chunk.instructions.len() as i32;
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
            }
            Statement::ForOf { left, right, body } => {
                self.symbols.enter_scope();
                self.compile_expression(right)?;
                let arr_slot = self.symbols.define_local(format!("@arr_{}", self.chunk.instructions.len()));
                self.chunk.emit(OpCode::SetLocal(arr_slot), 0);
                self.chunk.emit(OpCode::Pop, 0);

                let zero_idx = self.chunk.add_constant(Constant::Number(0.0));
                self.chunk.emit(OpCode::LoadConst(zero_idx), 0);
                let idx_slot = self.symbols.define_local(format!("@idx_{}", self.chunk.instructions.len()));
                self.chunk.emit(OpCode::SetLocal(idx_slot), 0);
                self.chunk.emit(OpCode::Pop, 0);

                let loop_start = self.chunk.instructions.len() as i32;
                self.chunk.emit(OpCode::GetLocal(idx_slot), 0);
                self.chunk.emit(OpCode::GetLocal(arr_slot), 0);
                let len_idx = self.chunk.add_constant(Constant::String("length".to_string()));
                self.chunk.emit(OpCode::GetProp(len_idx), 0);
                self.chunk.emit(OpCode::Lt, 0);
                let jump_exit_idx = self.chunk.emit(OpCode::JumpIfFalse(0), 0);

                self.chunk.emit(OpCode::GetLocal(arr_slot), 0);
                self.chunk.emit(OpCode::GetLocal(idx_slot), 0);
                self.chunk.emit(OpCode::GetElem, 0);

                if let Some(slot) = self.symbols.resolve_local(left) {
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    let name_idx = self.chunk.add_constant(Constant::String(left.clone()));
                    self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }

                let loop_labels = std::mem::take(&mut self.current_labels);
                self.break_targets.push(TargetScope { labels: loop_labels.clone(), jumps: Vec::new() });
                self.continue_targets.push(TargetScope { labels: loop_labels, jumps: Vec::new() });

                self.compile_statement(body, false)?;

                let update_start = self.chunk.instructions.len() as i32;
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
            }
            Statement::Labeled { label, body } => {
                self.current_labels.push(label.clone());
                let is_loop = matches!(body.as_ref(),
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
                    self.break_targets.push(TargetScope { labels, jumps: Vec::new() });
                    self.compile_statement(body, is_last)?;
                    let exit_target = self.chunk.instructions.len() as i32;
                    let breaks = self.break_targets.pop().unwrap();
                    for b in breaks.jumps {
                        self.chunk.patch_jump(b, exit_target);
                    }
                }
            }
            Statement::Switch { discriminant, cases } => {
                self.symbols.enter_scope();
                self.compile_expression(discriminant)?;
                let disc_slot = self.symbols.define_local(format!("@switch_{}", self.chunk.instructions.len()));
                self.chunk.emit(OpCode::SetLocal(disc_slot), 0);
                self.chunk.emit(OpCode::Pop, 0);

                self.break_targets.push(TargetScope { labels: Vec::new(), jumps: Vec::new() });

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
            }
            Statement::Break(opt_label) => {
                let jmp = self.chunk.emit(OpCode::Jump(0), 0);
                if let Some(label) = opt_label {
                    if let Some(target) = self.break_targets.iter_mut().rev().find(|t| t.labels.iter().any(|l| l == label)) {
                        target.jumps.push(jmp);
                    }
                } else if let Some(target) = self.break_targets.last_mut() {
                    target.jumps.push(jmp);
                }
            }
            Statement::Continue(opt_label) => {
                let jmp = self.chunk.emit(OpCode::Jump(0), 0);
                if let Some(label) = opt_label {
                    if let Some(target) = self.continue_targets.iter_mut().rev().find(|t| t.labels.iter().any(|l| l == label)) {
                        target.jumps.push(jmp);
                    }
                } else if let Some(target) = self.continue_targets.last_mut() {
                    target.jumps.push(jmp);
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
                        self.chunk.emit(OpCode::Pop, 0);
                    } else {
                        self.chunk.emit(OpCode::Pop, 0);
                    }
                    if !self.is_in_function {
                        self.chunk.emit(OpCode::LoadUndefined, 0);
                    }
                    self.compile_statement(catch_stmt, false)?;
                    self.symbols.exit_scope();
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
                let def = Self::compile_function(Some(name.clone()), params.clone(), body, self.is_strict)?;
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
                let def = Self::compile_function(Some(name.clone()), params.clone(), body, self.is_strict)?;
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
                let def = Self::compile_function(Some(name.clone()), params.clone(), body, self.is_strict)?;
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
            Statement::ClassDecl(class_def) => {
                self.compile_class_definition(class_def)?;
                if let Some(ref n) = class_def.name {
                    if self.symbols.scope_depth > 0 {
                        let slot = self.symbols.define_local(n.clone());
                        self.chunk.emit(OpCode::SetLocal(slot), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                    } else {
                        let name_idx = self.chunk.add_constant(Constant::String(n.clone()));
                        self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                    }
                } else {
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
            Statement::Empty | Statement::Debugger => {}
            Statement::With { object, body } => {
                self.compile_expression(object)?;
                self.chunk.emit(OpCode::PushScope, 0);
                self.with_depth += 1;
                self.compile_statement(body, is_last)?;
                self.with_depth -= 1;
                self.chunk.emit(OpCode::PopScope, 0);
            }
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
                Literal::String { value, .. } => {
                    let idx = self.chunk.add_constant(Constant::String(value.clone()));
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
                Literal::RegExp { pattern, flags } => {
                    let p_idx = self.chunk.add_constant(Constant::String(pattern.clone()));
                    let f_idx = self.chunk.add_constant(Constant::String(flags.clone()));
                    self.chunk.emit(OpCode::CreateRegExp(p_idx, f_idx), 0);
                }
            },
            Expression::Identifier(name) => {
                if self.with_depth == 0 {
                    if let Some(slot) = self.symbols.resolve_local(name) {
                        self.chunk.emit(OpCode::GetLocal(slot), 0);
                        return Ok(());
                    }
                }
                let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::GetGlobal(name_idx), 0);
            }
            Expression::This => {
                self.chunk.emit(OpCode::This, 0);
            }
            Expression::Assignment { target, value } => {
                self.compile_expression(value)?;
                self.chunk.emit(OpCode::Dup, 0);
                if self.with_depth == 0 {
                    if let Some(slot) = self.symbols.resolve_local(target) {
                        self.chunk.emit(OpCode::SetLocal(slot), 0);
                        self.chunk.emit(OpCode::Pop, 0);
                        return Ok(());
                    }
                }
                let name_idx = self.chunk.add_constant(Constant::String(target.clone()));
                self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                self.chunk.emit(OpCode::Pop, 0);
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
            Expression::Update { op, is_prefix, expr } => {
                let delta_is_pos = *op == UpdateOp::Inc;
                let one_idx = self.chunk.add_constant(Constant::Number(1.0));
                match expr.as_ref() {
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

                        if *is_prefix {
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
                        self.compile_expression(object)?;
                        self.chunk.emit(OpCode::Dup, 0);
                        let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                        self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                        self.chunk.emit(OpCode::Pos, 0);
                        if *is_prefix {
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
                    _ => return Err(CompileError::General("Invalid assignment target in update expression".into())),
                }
            }
            Expression::Object(props) => {
                self.chunk.emit(OpCode::CreateObject, 0);
                for prop in props {
                    match prop {
                        ObjectProperty::KeyValue(key, val_expr) => {
                            match key {
                                PropertyKey::Literal(k) => {
                                    self.chunk.emit(OpCode::Dup, 0);
                                    self.compile_expression(val_expr)?;
                                    let prop_idx = self.chunk.add_constant(Constant::String(k.clone()));
                                    self.chunk.emit(OpCode::SetProp(prop_idx), 0);
                                    self.chunk.emit(OpCode::Pop, 0);
                                }
                                PropertyKey::Computed(k_expr) => {
                                    self.chunk.emit(OpCode::Dup, 0);
                                    self.compile_expression(k_expr)?;
                                    self.compile_expression(val_expr)?;
                                    self.chunk.emit(OpCode::SetElem, 0);
                                    self.chunk.emit(OpCode::Pop, 0);
                                }
                            }
                        }
                        ObjectProperty::Getter(key, fn_expr) => {
                            match key {
                                PropertyKey::Literal(k) => {
                                    self.chunk.emit(OpCode::Dup, 0);
                                    self.compile_expression(fn_expr)?;
                                    let prop_idx = self.chunk.add_constant(Constant::String(k.clone()));
                                    self.chunk.emit(OpCode::DefineGetter(prop_idx), 0);
                                }
                                PropertyKey::Computed(k_expr) => {
                                    self.chunk.emit(OpCode::Dup, 0);
                                    self.compile_expression(k_expr)?;
                                    self.compile_expression(fn_expr)?;
                                    self.chunk.emit(OpCode::DefineComputedGetter, 0);
                                }
                            }
                        }
                        ObjectProperty::Setter(key, fn_expr) => {
                            match key {
                                PropertyKey::Literal(k) => {
                                    self.chunk.emit(OpCode::Dup, 0);
                                    self.compile_expression(fn_expr)?;
                                    let prop_idx = self.chunk.add_constant(Constant::String(k.clone()));
                                    self.chunk.emit(OpCode::DefineSetter(prop_idx), 0);
                                }
                                PropertyKey::Computed(k_expr) => {
                                    self.chunk.emit(OpCode::Dup, 0);
                                    self.compile_expression(k_expr)?;
                                    self.compile_expression(fn_expr)?;
                                    self.chunk.emit(OpCode::DefineComputedSetter, 0);
                                }
                            }
                        }
                    }
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
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing => unreachable!(),
                };
            }
            Expression::Unary { op, expr } => {
                if *op == UnaryOp::Typeof {
                    if let Expression::Identifier(name) = &**expr {
                        if let Some(slot) = self.symbols.resolve_local(name) {
                            self.chunk.emit(OpCode::GetLocal(slot), 0);
                        } else {
                            let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                            self.chunk.emit(OpCode::GetGlobalSafe(name_idx), 0);
                        }
                        self.chunk.emit(OpCode::Typeof, 0);
                        return Ok(());
                    }
                }
                if *op == UnaryOp::Delete {
                    match expr.as_ref() {
                        Expression::Member { object, property } => {
                            self.compile_expression(object)?;
                            let name_idx = self.chunk.add_constant(Constant::String(property.clone()));
                            self.chunk.emit(OpCode::DeleteProp(name_idx), 0);
                        }
                        Expression::Index { object, index } => {
                            self.compile_expression(object)?;
                            self.compile_expression(index)?;
                            self.chunk.emit(OpCode::DeleteElem, 0);
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
                        if matches!(object.as_ref(), Expression::Super) {
                            self.chunk.emit(OpCode::This, 0);
                            self.chunk.emit(OpCode::LoadSuper, 0);
                            let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                            self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        } else {
                            self.compile_expression(object)?;
                            self.chunk.emit(OpCode::Dup, 0);
                            let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                            self.chunk.emit(OpCode::GetProp(prop_idx), 0);
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        }
                    }
                    Expression::Index { object, index } => {
                        if matches!(object.as_ref(), Expression::Super) {
                            self.chunk.emit(OpCode::This, 0);
                            self.chunk.emit(OpCode::LoadSuper, 0);
                            self.compile_expression(index)?;
                            self.chunk.emit(OpCode::GetElem, 0);
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        } else {
                            self.compile_expression(object)?;
                            self.chunk.emit(OpCode::Dup, 0);
                            self.compile_expression(index)?;
                            self.chunk.emit(OpCode::GetElem, 0);
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        }
                    }
                    Expression::Super => {
                        if let Some(slot) = self.symbols.resolve_local("super") {
                            self.chunk.emit(OpCode::GetLocal(slot), 0);
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::Call(args.len() as u8), 0);
                        } else {
                            self.chunk.emit(OpCode::This, 0);
                            self.chunk.emit(OpCode::LoadSuper, 0);
                            for arg in args {
                                self.compile_expression(arg)?;
                            }
                            self.chunk.emit(OpCode::CallWithThis(args.len() as u8), 0);
                        }
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
                let def = Self::compile_function(name.clone(), params.clone(), body, self.is_strict)?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
            }
            Expression::GeneratorExpr { name, params, body } => {
                let def = Self::compile_function(name.clone(), params.clone(), body, self.is_strict)?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                self.chunk.emit(OpCode::CreateGenerator(const_idx), 0);
            }
            Expression::Super => {
                self.chunk.emit(OpCode::LoadSuper, 0);
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
            Expression::Sequence(exprs) => {
                let len = exprs.len();
                for (i, e) in exprs.iter().enumerate() {
                    self.compile_expression(e)?;
                    if i + 1 < len {
                        self.chunk.emit(OpCode::Pop, 0);
                    }
                }
            }
            Expression::ClassExpr(class_def) => {
                self.compile_class_definition(class_def)?;
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
