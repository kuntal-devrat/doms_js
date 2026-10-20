use doms_parser::ast::*;

pub(crate) fn collect_var_names(stmts: &[Statement], names: &mut Vec<String>) {
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
            Statement::ForIn { var_kind, left, body, .. }
            | Statement::ForOf { var_kind, left, body, .. } => {
                if *var_kind == Some(VarKind::Var) && !names.contains(left) {
                    names.push(left.clone());
                }
                collect_var_names(std::slice::from_ref(body), names);
            }
            Statement::Export { decl: Some(inner), .. } => {
                collect_var_names(std::slice::from_ref(inner), names);
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

pub(crate) fn collect_lexical_names(stmts: &[Statement], names: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            Statement::VarDecl { kind, declarations } if *kind != VarKind::Var => {
                for decl in declarations {
                    if !names.contains(&decl.name) {
                        names.push(decl.name.clone());
                    }
                }
            }
            Statement::ClassDecl(class_def) => {
                if let Some(ref name) = class_def.name {
                    if !names.contains(name) {
                        names.push(name.clone());
                    }
                }
            }
            Statement::Export { decl: Some(inner), .. } => {
                collect_lexical_names(std::slice::from_ref(inner), names);
            }
            Statement::Block(inner) => collect_lexical_names(inner, names),
            Statement::If { then_branch, else_branch, .. } => {
                collect_lexical_names(std::slice::from_ref(then_branch), names);
                if let Some(alt) = else_branch {
                    collect_lexical_names(std::slice::from_ref(alt), names);
                }
            }
            Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                collect_lexical_names(std::slice::from_ref(body), names);
            }
            Statement::For { init, body, .. } => {
                if let Some(init_stmt) = init {
                    collect_lexical_names(std::slice::from_ref(init_stmt), names);
                }
                collect_lexical_names(std::slice::from_ref(body), names);
            }
            Statement::ForIn { var_kind, left, body, .. }
            | Statement::ForOf { var_kind, left, body, .. } => {
                if matches!(var_kind, Some(VarKind::Let) | Some(VarKind::Const))
                    && !names.contains(left)
                {
                    names.push(left.clone());
                }
                collect_lexical_names(std::slice::from_ref(body), names);
            }
            Statement::With { body, .. } => {
                collect_lexical_names(std::slice::from_ref(body), names);
            }
            Statement::TryCatch { try_block, catch_block, finally_block, .. } => {
                collect_lexical_names(std::slice::from_ref(try_block), names);
                if let Some(cb) = catch_block {
                    collect_lexical_names(std::slice::from_ref(cb), names);
                }
                if let Some(fb) = finally_block {
                    collect_lexical_names(std::slice::from_ref(fb), names);
                }
            }
            Statement::Labeled { body, .. } => {
                collect_lexical_names(std::slice::from_ref(body), names);
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    collect_lexical_names(&case.consequent, names);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn is_param_default_stmt(stmt: &Statement) -> bool {
    match stmt {
        Statement::If { condition, then_branch, else_branch: None } => {
            if let Expression::Binary { op: BinaryOp::StrictEq, left, right } = condition {
                if let (Expression::Identifier(_), Expression::Literal(Literal::Undefined)) =
                    (&**left, &**right)
                {
                    if let Statement::Expression(Expression::Assignment { .. }) = &**then_branch {
                        return true;
                    }
                }
            }
            if let Expression::Binary { op: BinaryOp::Eq, left, right } = condition {
                if let (Expression::Identifier(_), Expression::Literal(Literal::Null)) =
                    (&**left, &**right)
                {
                    if let Statement::Throw(_) = &**then_branch {
                        return true;
                    }
                }
            }
            false
        }
        Statement::VarDecl { declarations, .. } => declarations.iter().any(|d| {
            d.name.starts_with("__destruct_")
                || d.name.starts_with("@destruct_")
                || d.name.starts_with("__param_")
                || d.name.starts_with("@param_")
                || d.name.starts_with("@dstr_")
        }),
        _ => false,
    }
}
