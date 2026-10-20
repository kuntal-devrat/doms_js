use super::Compiler;
use super::scope_analysis::collect_var_names;
use crate::bytecode::{BytecodeChunk, Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;
use std::sync::Arc;

impl Compiler {
    pub fn compile(mut self, program: &Program) -> Result<BytecodeChunk, CompileError> {
        let has_directive = program.body.iter()
            .take_while(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { .. }))))
            .any(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { value, has_escape: false, .. })) if value == "use strict"));
        if has_directive {
            self.is_strict = true;
        }
        self.symbols.enter_scope();

        // Collect top-level declarations for script GlobalDeclarationInstantiation
        let mut top_vars = Vec::new();
        collect_var_names(&program.body, &mut top_vars);

        let mut top_lex_names = Vec::new();
        let mut top_func_names = Vec::new();

        for stmt in &program.body {
            let decl_stmt = match stmt {
                Statement::Export { decl: Some(inner), .. } => &**inner,
                other => other,
            };
            match decl_stmt {
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
                | Statement::AsyncFunctionDecl { name, .. }
                | Statement::GeneratorDecl { name, .. }
                | Statement::AsyncGeneratorDecl { name, .. } => {
                    if !name.is_empty() {
                        top_func_names.push(name.clone());
                    }
                }
                _ => {}
            }
            if let Statement::Import { specifiers, .. } = stmt {
                for (name, alias) in specifiers {
                    let bound = alias.as_ref().unwrap_or(name);
                    top_lex_names.push(bound.clone());
                }
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

        let mut export_names = Vec::new();
        for stmt in &program.body {
            if let Statement::Export { decl, is_default, specifiers, all_as, .. } = stmt {
                if *is_default {
                    export_names.push("default".to_string());
                }
                if let Some(alias) = all_as {
                    export_names.push(alias.clone());
                }
                for (name, alias) in specifiers {
                    let exp_name = alias.as_ref().unwrap_or(name);
                    export_names.push(exp_name.clone());
                }
                if !*is_default {
                    if let Some(inner) = decl {
                        match &**inner {
                            Statement::VarDecl { declarations, .. } => {
                                for d in declarations {
                                    if !d.name.starts_with('@') {
                                        export_names.push(d.name.clone());
                                    }
                                }
                            }
                            Statement::ClassDecl(c) => {
                                if let Some(ref n) = c.name {
                                    export_names.push(n.clone());
                                }
                            }
                            Statement::FunctionDecl { name, .. }
                            | Statement::AsyncFunctionDecl { name, .. }
                            | Statement::GeneratorDecl { name, .. }
                            | Statement::AsyncGeneratorDecl { name, .. } => {
                                if !name.is_empty() {
                                    export_names.push(name.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        self.chunk.export_names = export_names;

        // Hoist imports and indirect re-exports
        for stmt in &program.body {
            match stmt {
                Statement::Import { .. } => {
                    self.compile_statement(stmt, false)?;
                }
                Statement::Export { source: Some(_), .. } => {
                    self.compile_statement(stmt, false)?;
                }
                _ => {}
            }
        }

        // Hoist top-level function declarations
        for stmt in &program.body {
            let func_info = match stmt {
                Statement::FunctionDecl { name, params, body } => {
                    Some((name, params, body, false, false, None))
                }
                Statement::AsyncFunctionDecl { name, params, body } => {
                    Some((name, params, body, true, false, None))
                }
                Statement::GeneratorDecl { name, params, body } => {
                    Some((name, params, body, false, true, None))
                }
                Statement::AsyncGeneratorDecl { name, params, body } => {
                    Some((name, params, body, true, true, None))
                }
                Statement::Export { decl: Some(inner), is_default, .. } => match &**inner {
                    Statement::FunctionDecl { name, params, body } => {
                        Some((name, params, body, false, false, Some(*is_default)))
                    }
                    Statement::AsyncFunctionDecl { name, params, body } => {
                        Some((name, params, body, true, false, Some(*is_default)))
                    }
                    Statement::GeneratorDecl { name, params, body } => {
                        Some((name, params, body, false, true, Some(*is_default)))
                    }
                    Statement::AsyncGeneratorDecl { name, params, body } => {
                        Some((name, params, body, true, true, Some(*is_default)))
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some((name, params, body, is_async, is_gen, opt_default)) = func_info {
                let def = Self::compile_function_full(
                    if name.is_empty() {
                        if opt_default == Some(true) { Some("default".to_string()) } else { None }
                    } else {
                        Some(name.clone())
                    },
                    params.clone(),
                    body,
                    self.is_strict,
                    is_gen,
                    is_async,
                    false,
                )?;
                let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                if is_async && is_gen {
                    self.chunk.emit(OpCode::CreateAsyncGenerator(const_idx), 0);
                } else if is_async {
                    self.chunk.emit(OpCode::CreateAsyncFunction(const_idx), 0);
                } else if is_gen {
                    self.chunk.emit(OpCode::CreateGenerator(const_idx), 0);
                } else {
                    self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                }
                if let Some(is_dflt) = opt_default {
                    if is_dflt {
                        self.chunk.emit(OpCode::Dup, 0);
                        let dflt_idx =
                            self.chunk.add_constant(Constant::String("default".to_string()));
                        self.chunk.emit(OpCode::Export(dflt_idx), 0);
                    }
                    if !name.is_empty() && !is_dflt {
                        self.chunk.emit(OpCode::Dup, 0);
                        let exp_idx = self.chunk.add_constant(Constant::String(name.clone()));
                        self.chunk.emit(OpCode::Export(exp_idx), 0);
                    }
                }
                if !name.is_empty() {
                    let slot = self.symbols.define_local(name.clone());
                    self.chunk.emit(OpCode::Dup, 0);
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                    let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    self.chunk.emit(OpCode::SetGlobal(name_idx), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                } else {
                    self.chunk.emit(OpCode::Pop, 0);
                }
            }
        }

        self.chunk.emit(OpCode::LoadUndefined, 0);
        for stmt in &program.body {
            match stmt {
                Statement::Import { .. } => continue,
                Statement::Export { source: Some(_), .. } => continue,
                Statement::FunctionDecl { .. }
                | Statement::AsyncFunctionDecl { .. }
                | Statement::GeneratorDecl { .. }
                | Statement::AsyncGeneratorDecl { .. } => continue,
                Statement::Export { decl: Some(inner), .. } => match &**inner {
                    Statement::FunctionDecl { .. }
                    | Statement::AsyncFunctionDecl { .. }
                    | Statement::GeneratorDecl { .. }
                    | Statement::AsyncGeneratorDecl { .. } => continue,
                    _ => {}
                },
                _ => {}
            }
            self.compile_statement(stmt, false)?;
        }
        self.chunk.emit(OpCode::Halt, 0);
        let local_names_arc = Arc::new(self.symbols.all_local_names.clone());
        self.chunk.local_names = Arc::clone(&local_names_arc);
        self.chunk.lexical_local_names = local_names_arc;
        Ok(self.chunk)
    }
}
