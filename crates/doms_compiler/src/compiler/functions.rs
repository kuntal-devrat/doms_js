use super::Compiler;
use super::scope_analysis::{collect_lexical_names, collect_var_names, is_param_default_stmt};
use crate::bytecode::{Constant, FunctionDef, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;
use std::sync::Arc;

impl Compiler {
    pub fn compile_function(
        name: Option<String>,
        params: Vec<String>,
        body: &[Statement],
        outer_is_strict: bool,
    ) -> Result<FunctionDef, CompileError> {
        Self::compile_function_full(name, params, body, outer_is_strict, false, false, false)
    }

    pub fn compile_function_full(
        name: Option<String>,
        params: Vec<String>,
        body: &[Statement],
        outer_is_strict: bool,
        is_generator: bool,
        is_arrow: bool,
        is_method: bool,
    ) -> Result<FunctionDef, CompileError> {
        let has_directive = body.iter()
            .take_while(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { .. }))))
            .any(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { value, has_escape: false, .. })) if value == "use strict"));
        let is_strict = outer_is_strict || has_directive;

        let mut compiler = Compiler::new();
        compiler.is_strict = is_strict;
        compiler.is_in_function = true;
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
                        if let (Expression::Identifier(id), Expression::Literal(Literal::Undefined)) =
                            (&**left, &**right)
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
        })
    }
}
