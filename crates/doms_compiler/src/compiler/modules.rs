use super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;
use std::sync::Arc;

impl Compiler {
    pub(crate) fn compile_import(
        &mut self,
        specifiers: &[(String, Option<String>)],
        source: &str,
    ) -> Result<(), CompileError> {
        let src_idx = self.chunk.add_constant(Constant::String(source.to_string()));
        self.chunk.emit(OpCode::ImportModule(src_idx), 0);
        for (name, alias) in specifiers {
            let bind_name = alias.as_ref().unwrap_or(name);
            self.chunk.imported_bindings.insert(bind_name.clone());
            let bind_idx = self.chunk.add_constant(Constant::String(bind_name.clone()));
            let exp_idx = self.chunk.add_constant(Constant::String(name.clone()));
            self.chunk.emit(OpCode::BindImport(bind_idx, exp_idx), 0);
        }
        self.chunk.emit(OpCode::Pop, 0);
        Ok(())
    }

    pub(crate) fn compile_export(
        &mut self,
        decl: &Option<Box<Statement>>,
        is_default: bool,
        specifiers: &[(String, Option<String>)],
        source: &Option<String>,
        is_all: bool,
        all_as: &Option<String>,
    ) -> Result<(), CompileError> {
        if let Some(src) = source {
            let src_idx = self.chunk.add_constant(Constant::String(src.clone()));
            self.chunk.emit(OpCode::ImportModule(src_idx), 0);
            if is_all {
                if let Some(alias) = all_as {
                    let name_idx = self.chunk.add_constant(Constant::String(alias.clone()));
                    self.chunk.emit(OpCode::Export(name_idx), 0);
                } else {
                    self.chunk.emit(OpCode::ExportAll, 0);
                }
            } else {
                for (name, alias) in specifiers {
                    let exp_name = alias.as_ref().unwrap_or(name);
                    let target_idx = self.chunk.add_constant(Constant::String(name.clone()));
                    let exp_idx = self.chunk.add_constant(Constant::String(exp_name.clone()));
                    self.chunk.emit(OpCode::ReExport(target_idx, exp_idx), 0);
                }
                self.chunk.emit(OpCode::Pop, 0);
            }
        } else if !specifiers.is_empty() {
            for (name, alias) in specifiers {
                let exp_name = alias.as_ref().unwrap_or(name);
                let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::GetGlobal(name_idx), 0);
                let exp_idx = self.chunk.add_constant(Constant::String(exp_name.clone()));
                self.chunk.emit(OpCode::Export(exp_idx), 0);
                if name != exp_name {
                    self.chunk
                        .export_aliases
                        .entry(name.clone())
                        .or_default()
                        .push(exp_name.clone());
                }
            }
        } else if let Some(stmt) = decl {
            match &**stmt {
                Statement::VarDecl { declarations, .. } => {
                    self.compile_statement(stmt, false)?;
                    for decl in declarations {
                        if !decl.name.starts_with('@') {
                            let name_idx =
                                self.chunk.add_constant(Constant::String(decl.name.clone()));
                            self.chunk.emit(OpCode::GetGlobalSafe(name_idx), 0);
                            self.chunk.emit(OpCode::Export(name_idx), 0);
                        }
                    }
                }
                Statement::FunctionDecl { name, params, body }
                | Statement::AsyncFunctionDecl { name, params, body }
                | Statement::GeneratorDecl { name, params, body }
                | Statement::AsyncGeneratorDecl { name, params, body } => {
                    if is_default && name.is_empty() {
                        let is_async = matches!(
                            &**stmt,
                            Statement::AsyncFunctionDecl { .. }
                                | Statement::AsyncGeneratorDecl { .. }
                        );
                        let is_gen = matches!(
                            &**stmt,
                            Statement::GeneratorDecl { .. } | Statement::AsyncGeneratorDecl { .. }
                        );
                        let def = Self::compile_function_full(
                            Some("default".to_string()),
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
                        let def_idx =
                            self.chunk.add_constant(Constant::String("default".to_string()));
                        self.chunk.emit(OpCode::Export(def_idx), 0);
                    } else {
                        self.compile_statement(stmt, false)?;
                        if is_default {
                            let def_idx =
                                self.chunk.add_constant(Constant::String("default".to_string()));
                            if !name.is_empty() {
                                let id_idx =
                                    self.chunk.add_constant(Constant::String(name.clone()));
                                self.chunk.emit(OpCode::GetGlobalSafe(id_idx), 0);
                                self.chunk
                                    .export_aliases
                                    .entry(name.clone())
                                    .or_default()
                                    .push("default".to_string());
                            }
                            self.chunk.emit(OpCode::Export(def_idx), 0);
                        } else {
                            let name_idx = self.chunk.add_constant(Constant::String(name.clone()));
                            self.chunk.emit(OpCode::GetGlobalSafe(name_idx), 0);
                            self.chunk.emit(OpCode::Export(name_idx), 0);
                        }
                    }
                }
                Statement::ClassDecl(class_def) => {
                    if is_default && class_def.name.is_none() {
                        self.compile_class_definition_with_name(
                            class_def,
                            Some("default".to_string()),
                            false,
                        )?;
                        let def_idx =
                            self.chunk.add_constant(Constant::String("default".to_string()));
                        self.chunk.emit(OpCode::Export(def_idx), 0);
                    } else {
                        self.compile_statement(stmt, false)?;
                        if is_default {
                            let def_idx =
                                self.chunk.add_constant(Constant::String("default".to_string()));
                            if let Some(ref n) = class_def.name {
                                let id_idx = self.chunk.add_constant(Constant::String(n.clone()));
                                self.chunk.emit(OpCode::GetGlobalSafe(id_idx), 0);
                            }
                            self.chunk.emit(OpCode::Export(def_idx), 0);
                        }
                        if let Some(ref n) = class_def.name {
                            if !is_default {
                                let name_idx = self.chunk.add_constant(Constant::String(n.clone()));
                                self.chunk.emit(OpCode::GetGlobalSafe(name_idx), 0);
                                self.chunk.emit(OpCode::Export(name_idx), 0);
                            }
                        }
                    }
                }
                Statement::Expression(expr) if is_default => {
                    let mut expr_cloned = expr.clone();
                    fn set_default_name(e: &mut Expression) {
                        match e {
                            Expression::FunctionExpr { name, .. } if name.is_none() => {
                                *name = Some("default".to_string())
                            }
                            Expression::AsyncFunctionExpr { name, .. } if name.is_none() => {
                                *name = Some("default".to_string())
                            }
                            Expression::GeneratorExpr { name, .. } if name.is_none() => {
                                *name = Some("default".to_string())
                            }
                            Expression::AsyncGeneratorExpr { name, .. } if name.is_none() => {
                                *name = Some("default".to_string())
                            }
                            Expression::ClassExpr(class_def) if class_def.name.is_none() => {
                                class_def.name = Some("default".to_string())
                            }
                            Expression::Grouping(inner) => set_default_name(inner),
                            _ => {}
                        }
                    }
                    set_default_name(&mut expr_cloned);
                    self.compile_expression(&expr_cloned)?;
                    let name_idx = self.chunk.add_constant(Constant::String("default".to_string()));
                    self.chunk.emit(OpCode::Export(name_idx), 0);
                }
                _ => {
                    self.compile_statement(stmt, false)?;
                }
            }
        }
        Ok(())
    }
}
