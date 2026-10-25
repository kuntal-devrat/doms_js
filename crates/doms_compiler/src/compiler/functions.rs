use super::Compiler;
use super::scope_analysis::{collect_lexical_names, collect_var_names, is_param_default_stmt};
use crate::bytecode::{Constant, FunctionDef, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

impl Compiler {
    pub fn compile_function(
        name: Option<String>,
        params: Vec<String>,
        body: &[Statement],
        outer_is_strict: bool,
    ) -> Result<FunctionDef, CompileError> {
        Self::compile_function_full(
            name,
            params,
            body,
            outer_is_strict,
            false,
            false,
            false,
            HashSet::new(),
            HashMap::new(),
        )
    }

    pub fn compile_function_full(
        name: Option<String>,
        params: Vec<String>,
        body: &[Statement],
        outer_is_strict: bool,
        is_generator: bool,
        is_arrow: bool,
        is_method: bool,
        imported_bindings: HashSet<String>,
        export_aliases: HashMap<String, Vec<String>>,
    ) -> Result<FunctionDef, CompileError> {
        Self::compile_function_internal(
            name,
            params,
            body,
            outer_is_strict,
            is_generator,
            is_arrow,
            is_method,
            false,
            false,
            imported_bindings,
            export_aliases,
        )
    }

    pub fn compile_class_constructor(
        name: Option<String>,
        params: Vec<String>,
        body: &[Statement],
        is_derived: bool,
        imported_bindings: HashSet<String>,
        export_aliases: HashMap<String, Vec<String>>,
    ) -> Result<FunctionDef, CompileError> {
        Self::compile_function_internal(
            name,
            params,
            body,
            true,
            false,
            false,
            false,
            true,
            is_derived,
            imported_bindings,
            export_aliases,
        )
    }

    pub fn compile_derived_default_constructor(
        name: Option<String>,
        body: &[Statement],
        imported_bindings: HashSet<String>,
        export_aliases: HashMap<String, Vec<String>>,
    ) -> Result<FunctionDef, CompileError> {
        let mut compiler = Compiler::new();
        compiler.is_strict = true;
        compiler.is_in_function = true;
        compiler.chunk.imported_bindings = imported_bindings;
        compiler.chunk.export_aliases = export_aliases;
        compiler.symbols.enter_scope();

        compiler.chunk.emit(OpCode::SuperCallForward, 0);

        let total = body.len();
        for (i, stmt) in body.iter().enumerate() {
            let is_last = i == total - 1;
            compiler.compile_statement(stmt, is_last)?;
        }

        compiler.chunk.emit(OpCode::LoadUndefined, 0);
        compiler.chunk.emit(OpCode::Return, 0);

        let local_names_arc = std::sync::Arc::new(compiler.symbols.all_local_names);
        compiler.chunk.local_names = std::sync::Arc::clone(&local_names_arc);

        Ok(FunctionDef {
            name,
            params: Vec::new(),
            length: 0,
            self_slot: None,
            local_names: local_names_arc,
            lexical_local_names: Arc::new(Vec::new()),
            chunk: compiler.chunk,
            is_strict: true,
            has_param_exprs: false,
            is_generator: false,
            is_arrow: false,
            is_method: false,
            is_class_constructor: true,
            is_derived_constructor: true,
        })
    }

    pub fn compile_function_internal(
        name: Option<String>,
        params: Vec<String>,
        body: &[Statement],
        outer_is_strict: bool,
        is_generator: bool,
        is_arrow: bool,
        is_method: bool,
        is_class_constructor: bool,
        is_derived_constructor: bool,
        imported_bindings: HashSet<String>,
        export_aliases: HashMap<String, Vec<String>>,
    ) -> Result<FunctionDef, CompileError> {
        let has_directive = body.iter()
            .take_while(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { .. }))))
            .any(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { value, has_escape: false, .. })) if value == "use strict"));
        let is_strict = outer_is_strict || has_directive;

        let mut compiler = Compiler::new();
        compiler.is_strict = is_strict;
        compiler.is_in_function = true;
        compiler.chunk.imported_bindings = imported_bindings;
        compiler.chunk.export_aliases = export_aliases;
        compiler.symbols.enter_scope();
        for p in &params {
            let local_name =
                if let Some(rest_name) = p.strip_prefix("...") { rest_name } else { p.as_str() };
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

        let mut self_slot = None;
        if let Some(ref fn_name) = name {
            if !fn_name.is_empty() && compiler.symbols.resolve_local(fn_name).is_none() {
                let slot = compiler.symbols.define_local_at_depth(fn_name.clone(), 1);
                self_slot = Some(slot);
            }
        }
        compiler.self_slot = self_slot;

        let mut lexical_local_names = Vec::new();
        collect_lexical_names(body, &mut lexical_local_names);

        let mut idx = 0;
        let mut has_param_exprs = false;
        while idx < body.len() {
            if matches!(&body[idx], Statement::Expression(Expression::Literal(Literal::String { value, has_escape: false, .. })) if value == "use strict")
            {
                idx += 1;
                continue;
            }
            if is_param_default_stmt(&body[idx]) {
                has_param_exprs = true;
                idx += 1;
                continue;
            }
            break;
        }
        if params.iter().any(|p| p.starts_with("__param_") || p.contains('=')) {
            has_param_exprs = true;
        }

        for stmt in &body[..idx] {
            if let Statement::If { condition, then_branch, else_branch: None } = stmt {
                if let Expression::Binary { op: BinaryOp::StrictEq, left, right } = condition {
                    if let (Expression::Identifier(id), Expression::Literal(Literal::Undefined)) =
                        (&**left, &**right)
                    {
                        if let Statement::Expression(Expression::Assignment { target, value }) =
                            &**then_branch
                        {
                            if id == target {
                                if let Some(slot) = compiler.symbols.resolve_local(target) {
                                    let jump_lbl =
                                        compiler.chunk.emit(OpCode::JumpIfLocalDefined(slot, 0), 0);
                                    compiler.compile_expression(value)?;
                                    compiler.chunk.emit(OpCode::SetLocal(slot), 0);
                                    compiler.chunk.emit(OpCode::Pop, 0);
                                    let target = compiler.chunk.instructions.len() as i32;
                                    compiler.chunk.patch_jump(jump_lbl, target);
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            compiler.compile_statement(stmt, false)?;
        }

        if has_param_exprs {
            compiler.chunk.emit(OpCode::EnterFunctionBody, 0);
        }

        if is_generator {
            compiler.chunk.emit(OpCode::GeneratorStart, 0);
        }

        // Hoist function declarations to the top of the function
        for stmt in &body[idx..] {
            if let Statement::FunctionDecl { name, params, body } = stmt {
                let def = Self::compile_function(
                    Some(name.clone()),
                    params.clone(),
                    body,
                    compiler.is_strict,
                )?;
                let const_idx = compiler.chunk.add_constant(Constant::Function(Arc::new(def)));
                compiler.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                let slot = compiler.symbols.define_local(name.clone());
                compiler.chunk.emit(OpCode::SetLocal(slot), 0);
                compiler.chunk.emit(OpCode::Pop, 0);
            }
        }

        let rest_body = &body[idx..];
        let total = rest_body.len();
        for (i, stmt) in rest_body.iter().enumerate() {
            if matches!(stmt, Statement::FunctionDecl { .. }) {
                continue;
            }
            let is_last = i == total - 1;
            compiler.compile_statement(stmt, is_last)?;
        }
        // Default return undefined
        compiler.chunk.emit(OpCode::LoadUndefined, 0);
        compiler.chunk.emit(OpCode::Return, 0);
        let mut is_top = vec![false; compiler.symbols.all_local_names.len()];
        for local in &compiler.symbols.locals {
            if (local.slot as usize) < is_top.len() {
                is_top[local.slot as usize] = true;
            }
        }
        compiler.chunk.is_top_local = std::sync::Arc::new(is_top);
        let local_names_arc = std::sync::Arc::new(compiler.symbols.all_local_names);
        let lexical_local_names_arc = std::sync::Arc::new(lexical_local_names);
        compiler.chunk.local_names = std::sync::Arc::clone(&local_names_arc);
        compiler.chunk.lexical_local_names = std::sync::Arc::clone(&lexical_local_names_arc);
        compiler.chunk.self_slot = self_slot;
        let mut length = 0;
        for p in &params {
            if p.starts_with("...") {
                break;
            }
            let is_param_with_default = body.iter().any(|s| {
                if let Statement::If { condition, .. } = s {
                    if let Expression::Binary { op: BinaryOp::StrictEq, left, right } = condition {
                        if let (
                            Expression::Identifier(id),
                            Expression::Literal(Literal::Undefined),
                        ) = (&**left, &**right)
                        {
                            if id == p {
                                return true;
                            }
                        }
                    }
                }
                false
            });
            if is_param_with_default || p.contains('=') {
                break;
            }
            length += 1;
        }

        Ok(FunctionDef {
            name,
            params,
            length,
            self_slot,
            local_names: local_names_arc,
            lexical_local_names: lexical_local_names_arc,
            chunk: compiler.chunk,
            is_strict,
            has_param_exprs,
            is_generator,
            is_arrow,
            is_method,
            is_class_constructor,
            is_derived_constructor,
        })
    }
}
