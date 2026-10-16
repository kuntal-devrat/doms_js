use super::{ParseError, Parser};
use crate::ast::*;

impl<'a> Parser<'a> {
    pub(crate) fn check_sub_statement(&self, stmt: &Statement, allow_sloppy_func: bool) -> Result<(), ParseError> {
        match stmt {
            Statement::VarDecl { kind: VarKind::Let | VarKind::Const, .. }
            | Statement::ClassDecl(_)
            | Statement::AsyncFunctionDecl { .. }
            | Statement::GeneratorDecl { .. }
            | Statement::AsyncGeneratorDecl { .. } => {
                Err(ParseError::General("Declaration cannot appear in a single-statement context".into()))
            }
            Statement::FunctionDecl { .. } => {
                if allow_sloppy_func && !self.is_strict {
                    Ok(())
                } else {
                    Err(ParseError::General("Function declaration cannot appear in a single-statement context".into()))
                }
            }
            Statement::Labeled { body, .. } => {
                self.check_sub_statement(body, allow_sloppy_func)
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn validate_block_declarations(&self, stmts: &[Statement]) -> Result<(), ParseError> {
        let mut lex_names: Vec<(String, bool)> = Vec::new();
        for stmt in stmts {
            match stmt {
                Statement::VarDecl { kind, declarations } if *kind != VarKind::Var => {
                    for decl in declarations {
                        if !decl.name.starts_with("__") {
                            Self::add_lexical_name(&mut lex_names, decl.name.clone(), false, self.is_strict)?;
                        }
                    }
                }
                Statement::ClassDecl(class_def) => {
                    if let Some(ref name) = class_def.name {
                        Self::add_lexical_name(&mut lex_names, name.clone(), false, self.is_strict)?;
                    }
                }
                Statement::FunctionDecl { name, .. } => {
                    Self::add_lexical_name(&mut lex_names, name.clone(), true, self.is_strict)?;
                }
                Statement::AsyncFunctionDecl { name, .. }
                | Statement::GeneratorDecl { name, .. }
                | Statement::AsyncGeneratorDecl { name, .. } => {
                    Self::add_lexical_name(&mut lex_names, name.clone(), false, self.is_strict)?;
                }
                _ => {}
            }
        }

        if !lex_names.is_empty() {
            let mut var_names = Vec::new();
            for stmt in stmts {
                Self::collect_var_declared_names(stmt, &mut var_names);
            }
            for (lex, _) in &lex_names {
                if var_names.contains(lex) {
                    return Err(ParseError::General(format!(
                        "Identifier '{}' has already been declared",
                        lex
                    )));
                }
            }
        }

        Ok(())
    }

    pub(crate) fn add_lexical_name(
        names: &mut Vec<(String, bool)>,
        name: String,
        is_plain_func: bool,
        is_strict: bool,
    ) -> Result<(), ParseError> {
        if let Some((_, prev_is_plain_func)) = names.iter().find(|(n, _)| *n == name) {
            if !is_strict && *prev_is_plain_func && is_plain_func {
                // Annex B allows duplicate ordinary FunctionDeclarations in sloppy mode
            } else {
                return Err(ParseError::General(format!(
                    "Identifier '{}' has already been declared",
                    name
                )));
            }
        }
        names.push((name, is_plain_func));
        Ok(())
    }

    pub(crate) fn collect_var_declared_names(stmt: &Statement, vars: &mut Vec<String>) {
        match stmt {
            Statement::VarDecl { kind: VarKind::Var, declarations } => {
                for decl in declarations {
                    if !vars.contains(&decl.name) {
                        vars.push(decl.name.clone());
                    }
                }
            }
            Statement::Block(inner) => {
                for s in inner {
                    Self::collect_var_declared_names(s, vars);
                }
            }
            Statement::If { then_branch, else_branch, .. } => {
                Self::collect_var_declared_names(then_branch, vars);
                if let Some(eb) = else_branch {
                    Self::collect_var_declared_names(eb, vars);
                }
            }
            Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                Self::collect_var_declared_names(body, vars);
            }
            Statement::For { init, body, .. } => {
                if let Some(init_stmt) = init {
                    Self::collect_var_declared_names(init_stmt, vars);
                }
                Self::collect_var_declared_names(body, vars);
            }
            Statement::ForIn { var_kind, left, body, .. } | Statement::ForOf { var_kind, left, body, .. } => {
                if *var_kind == Some(VarKind::Var) && !vars.contains(left) {
                    vars.push(left.clone());
                }
                Self::collect_var_declared_names(body, vars);
            }
            Statement::With { body, .. } | Statement::Labeled { body, .. } => {
                Self::collect_var_declared_names(body, vars);
            }
            Statement::TryCatch { try_block, catch_block, finally_block, .. } => {
                Self::collect_var_declared_names(try_block, vars);
                if let Some(cb) = catch_block {
                    Self::collect_var_declared_names(cb, vars);
                }
                if let Some(fb) = finally_block {
                    Self::collect_var_declared_names(fb, vars);
                }
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    for s in &case.consequent {
                        Self::collect_var_declared_names(s, vars);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn validate_labels_and_jumps(stmts: &[Statement]) -> Result<(), ParseError> {
        let mut active_labels = Vec::new();
        for stmt in stmts {
            Self::check_labels_and_jumps_stmt(stmt, &mut active_labels, false, false)?;
        }
        Ok(())
    }

    pub(crate) fn check_labels_and_jumps_stmt(
        stmt: &Statement,
        active_labels: &mut Vec<String>,
        in_loop: bool,
        in_switch: bool,
    ) -> Result<(), ParseError> {
        match stmt {
            Statement::Labeled { label, body } => {
                if active_labels.contains(label) {
                    return Err(ParseError::General(format!("Duplicate label '{}'", label)));
                }
                active_labels.push(label.clone());
                let res = Self::check_labels_and_jumps_stmt(body, active_labels, in_loop, in_switch);
                active_labels.pop();
                res?;
            }
            Statement::Break(Some(label)) => {
                if !active_labels.contains(label) {
                    return Err(ParseError::General(format!("Undefined break label '{}'", label)));
                }
            }
            Statement::Break(None) => {
                if !in_loop && !in_switch {
                    return Err(ParseError::General("Illegal break statement".into()));
                }
            }
            Statement::Continue(Some(label)) => {
                if !active_labels.contains(label) {
                    return Err(ParseError::General(format!("Undefined continue label '{}'", label)));
                }
                if !in_loop {
                    return Err(ParseError::General("Illegal continue statement".into()));
                }
            }
            Statement::Continue(None) => {
                if !in_loop {
                    return Err(ParseError::General("Illegal continue statement".into()));
                }
            }
            Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, true, in_switch)?;
            }
            Statement::For { body, .. }
            | Statement::ForIn { body, .. }
            | Statement::ForOf { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, true, in_switch)?;
            }
            Statement::Block(inner) => {
                for s in inner {
                    Self::check_labels_and_jumps_stmt(s, active_labels, in_loop, in_switch)?;
                }
            }
            Statement::If { then_branch, else_branch, .. } => {
                Self::check_labels_and_jumps_stmt(then_branch, active_labels, in_loop, in_switch)?;
                if let Some(eb) = else_branch {
                    Self::check_labels_and_jumps_stmt(eb, active_labels, in_loop, in_switch)?;
                }
            }
            Statement::Switch { cases, .. } => {
                for c in cases {
                    for s in &c.consequent {
                        Self::check_labels_and_jumps_stmt(s, active_labels, in_loop, true)?;
                    }
                }
            }
            Statement::TryCatch { try_block, catch_block, finally_block, .. } => {
                Self::check_labels_and_jumps_stmt(try_block, active_labels, in_loop, in_switch)?;
                if let Some(cb) = catch_block {
                    Self::check_labels_and_jumps_stmt(cb, active_labels, in_loop, in_switch)?;
                }
                if let Some(fb) = finally_block {
                    Self::check_labels_and_jumps_stmt(fb, active_labels, in_loop, in_switch)?;
                }
            }
            Statement::With { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, in_loop, in_switch)?;
            }
            _ => {}
        }
        Ok(())
    }
}
